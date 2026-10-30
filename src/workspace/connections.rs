use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    feedback::Alert,
    lists::List,
    menus::{Menu, MenuItem, OverflowMenu},
    motion::Reorder,
    primitives::{IconName, Severity},
    shell::ShellColumn,
    theme::ControlSize,
};
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, div,
};

use super::{Workspace, panel::tab_button, widths::Side};
use crate::{
    sessions,
    settings::SettingsStore,
    ssh::{Host, HostStore, control::Control},
    term::{config, spawn},
};

impl Workspace {
    pub(super) fn follow_monitor(&mut self, cx: &mut Context<Self>) {
        let target = self
            .sessions
            .get(self.active)
            .and_then(|session| {
                session
                    .remote
                    .as_ref()
                    .map(|host| crate::monitor::Target::Remote {
                        host: host.clone(),
                        socket: session
                            .control
                            .as_ref()
                            .and_then(|control| control.socket()),
                        disconnected: session.disconnected,
                    })
            })
            .unwrap_or(crate::monitor::Target::Local);
        self.monitor.update(cx, |panel, cx| panel.show(target, cx));
    }

    pub(super) fn connect(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        self.connect_at(host, None, window, cx);
    }

    pub(super) fn connect_at(
        &mut self,
        host: Host,
        folder: Option<crate::ssh::files::RemotePath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let started = (|| {
            let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("ssh: no home directory"))?;
            let mut launch = host.launch(home)?;
            let control = Control::new()?;
            control.configure(&mut launch);
            crate::ssh::shell::configure(&mut launch, folder.as_ref())?;
            let opened_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| anyhow::anyhow!("ssh: cannot read clock: {error}"))?
                .as_secs();
            let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
            HostStore::update(cx, |data| {
                data.opened(host.clone(), opened_at);
                Ok(())
            })?;
            Ok::<_, anyhow::Error>((pty, events, launch, control))
        })();
        match started {
            Ok((pty, events, launch, control)) => {
                self.ssh_error = None;
                self.add(pty, events, launch, window, cx);
                let index = self.sessions.len() - 1;
                self.sessions[index].name = host.label().to_string();
                self.sessions[index].remote = Some(host.clone());
                self.sessions[index].control = Some(control);
                self.sidebar_hosts = false;
                self.select(index, window, cx);
                log::info!("ssh: session starts for {}", host.destination());
            }
            Err(error) => {
                log::error!("ssh: connection failed: {error:#}");
                self.ssh_error = Some(format!("{error:#}"));
                self.sidebar_hosts = true;
                cx.notify();
            }
        }
    }

    pub(super) fn new_menu(&self, cx: &Context<Self>) -> OverflowMenu {
        let (local, quick, saved) = (cx.entity(), cx.entity(), cx.entity());
        let menu = Menu::new()
            .item(
                MenuItem::new("Local terminal")
                    .icon(IconName::Terminal)
                    .on_click(move |window, cx| {
                        local.update(cx, |workspace, cx| workspace.new_session(window, cx))
                    }),
            )
            .item(
                MenuItem::new("Quick SSH connection")
                    .icon(IconName::Zap)
                    .on_click(move |window, cx| {
                        quick.update(cx, |workspace, cx| workspace.edit_host(true, window, cx))
                    }),
            )
            .separator()
            .item(
                MenuItem::new("Add SSH host")
                    .icon(IconName::Server)
                    .on_click(move |window, cx| {
                        saved.update(cx, |workspace, cx| workspace.edit_host(false, window, cx))
                    }),
            );
        OverflowMenu::new("new-connection", menu)
            .icon(IconName::Plus)
            .tooltip("New connection")
    }

    fn edit_host(&mut self, quick: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_hosts = true;
        self.hosts.update(cx, |panel, cx| {
            panel.edit(Host::default(), quick, window, cx)
        });
        cx.notify();
    }

    pub(super) fn connection_sidebar(
        &self,
        left: IconButton,
        now: Instant,
        cx: &Context<Self>,
    ) -> ShellColumn {
        let switch = [
            (
                false,
                "connections-sessions",
                IconName::Terminal,
                "Sessions",
            ),
            (true, "connections-hosts", IconName::Server, "Hosts"),
        ]
        .into_iter()
        .map(|(hosts, id, icon, name)| {
            let owner = cx.entity();
            tab_button(id, icon, name, self.sidebar_hosts == hosts).on_click(move |_, _, cx| {
                owner.update(cx, |workspace, cx| {
                    if workspace.sidebar_hosts != hosts {
                        workspace.sidebar_hosts = hosts;
                        log::debug!("ssh: sidebar shows {name}");
                        cx.notify();
                    }
                })
            })
        });
        let home = dirs::home_dir();
        let workspace = cx.entity();
        let rows = self
            .sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                let row = sessions::row(
                    &workspace,
                    index,
                    session,
                    index == self.active,
                    home.as_deref(),
                    now,
                    cx,
                );
                (
                    session.terminal.entity_id().as_u64().to_string(),
                    row.into_any_element(),
                )
            })
            .collect::<Vec<_>>();
        let list = if SettingsStore::get(cx).drag_sessions {
            let dragged = workspace.clone();
            rows.into_iter()
                .fold(
                    Reorder::new("session-rows").on_reorder(move |from, to, _, cx| {
                        dragged.update(cx, |this, cx| this.reorder(from, to, cx))
                    }),
                    |list, (key, row)| list.row(key, div().pb_0p5().child(row)),
                )
                .into_any_element()
        } else {
            List::new()
                .children(rows.into_iter().map(|(_, row)| row))
                .into_any_element()
        };
        let error = self.ssh_error.as_ref().map(|error| {
            let dismiss = cx.entity();
            Alert::new("ssh-spawn-error", Severity::Danger, "SSH could not start")
                .body(error.clone())
                .on_dismiss(move |_, cx| {
                    dismiss.update(cx, |workspace, cx| {
                        workspace.ssh_error = None;
                        cx.notify();
                    })
                })
        });
        let body = if self.sidebar_hosts {
            self.hosts.clone().into_any_element()
        } else {
            div()
                .id("sessions")
                .size_full()
                .overflow_y_scroll()
                .child(list)
                .into_any_element()
        };
        let list = div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .justify_center()
                    .gap_1()
                    .px_2()
                    .py_2()
                    .children(switch),
            )
            .children(error)
            .child(div().flex_1().min_h_0().p_2().child(body));
        self.side_column(Side::Sessions, cx)
            .leading(left)
            .action(self.new_menu(cx))
            .child(list)
    }

    pub(super) fn reconnect_button(&self, cx: &Context<Self>) -> Option<IconButton> {
        let session = self.sessions.get(self.active)?;
        let host = session.remote.as_ref()?.clone();
        if !session.disconnected {
            return None;
        }
        let owner = cx.entity();
        Some(
            IconButton::new("ssh-reconnect", IconName::RefreshCw)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip("Reconnect")
                .on_click(move |_, window, cx| {
                    owner.update(cx, |workspace, cx| {
                        workspace.connect(host.clone(), window, cx);
                    })
                }),
        )
    }
}
