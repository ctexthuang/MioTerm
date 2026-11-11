use ely_gpui_component::{
    i18n::text,
    primitives::{Icon, IconName},
    theme::{ActiveTheme, TextSize},
};
use gpui::{AppContext, Context, Div, EntityId, ParentElement, Styled, Window, div};

use super::{Workspace, remote::endpoint};
use crate::{
    settings::SettingsStore,
    ssh::control::Control,
    term::{TerminalView, config, spawn},
};

impl Workspace {
    pub(super) fn schedule_reconnect(
        &mut self,
        id: EntityId,
        code: Option<i32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.terminal.entity_id() == id)
        else {
            return;
        };
        let enabled = session
            .remote
            .as_ref()
            .is_some_and(|host| host.auto_reconnect);
        let Some(delay) = session.recovery.next(enabled, code) else {
            return;
        };
        log::info!(
            "ssh: reconnect attempt {} scheduled in {}s",
            session.recovery.attempts,
            delay.as_secs()
        );
        session.reconnecting = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update_in(cx, |this, window, cx| {
                if this.sessions.iter().any(|session| {
                    session.terminal.entity_id() == id
                        && session.disconnected
                        && session.recovery.waiting
                }) {
                    this.reconnect(id, true, window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    pub(super) fn stop_reconnect(&mut self, id: EntityId, cx: &mut Context<Self>) {
        if let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.terminal.entity_id() == id)
        {
            session.reconnecting = None;
            session.recovery.waiting = false;
            if let Some(host) = &mut session.remote {
                host.auto_reconnect = false;
            }
            log::info!("ssh: automatic reconnect stopped for session {id:?}");
            cx.notify();
        }
    }

    pub fn reconnect(
        &mut self,
        id: EntityId,
        automatic: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.terminal.entity_id() == id && session.disconnected)
        else {
            return;
        };
        let Some(mut host) = self.sessions[index].remote.clone() else {
            return;
        };
        if let Some(saved) = crate::ssh::HostStore::get(cx).hosts.iter().find(|saved| {
            !host.sync.id.is_empty()
                && saved.sync.id == host.sync.id
                && saved.address == host.address
                && saved.port == host.port
                && saved.user == host.user
        }) {
            let auto_reconnect = host.auto_reconnect;
            host = saved.clone();
            host.auto_reconnect = auto_reconnect;
        }
        self.sessions[index].reconnecting = None;
        self.sessions[index].recovery.waiting = false;
        if !automatic {
            self.sessions[index].recovery = Default::default();
        }
        let opened = (|| {
            let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("ssh: no home directory"))?;
            let mut launch = host.launch(home)?;
            let control = Control::new()?;
            control.configure(&mut launch);
            crate::ssh::shell::configure(&mut launch, self.sessions[index].remote_folder.as_ref())?;
            if automatic && let Some((_, args)) = &mut launch.program {
                let batch = if host.remember_secret {
                    "BatchMode=no"
                } else {
                    "BatchMode=yes"
                };
                args.splice(
                    1..1,
                    [
                        "-o".into(),
                        "ConnectTimeout=5".into(),
                        "-o".into(),
                        "ConnectionAttempts=1".into(),
                        "-o".into(),
                        "StrictHostKeyChecking=yes".into(),
                        "-o".into(),
                        "NumberOfPasswordPrompts=1".into(),
                        "-o".into(),
                        batch.into(),
                    ],
                );
            }
            let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
            Ok::<_, anyhow::Error>((pty, events, launch, control))
        })();
        match opened {
            Ok((pty, events, launch, control)) => {
                let terminal = cx.new(|cx| TerminalView::new(pty, events, window, cx));
                let new_id = terminal.entity_id();
                let subscription =
                    cx.subscribe_in(&terminal, window, move |this, _, event, window, cx| {
                        this.session_event(new_id, event, window, cx)
                    });
                let session = &mut self.sessions[index];
                session
                    .terminal
                    .update(cx, |view, _| view.set_front(false, window));
                session.terminal = terminal;
                session._events = subscription;
                session.control = Some(control);
                session.remote = Some(host);
                session.disconnected = false;
                session.title.clear();
                session.shell = launch
                    .program
                    .map_or_else(String::new, |(program, _)| program);
                session.bell = false;
                session.waiting = false;
                session.look = None;
                session.editing = None;
                session.recovery.error = None;
                let remote = endpoint(session);
                self.transfers
                    .update(cx, |tasks, cx| tasks.bind(id.as_u64(), remote.as_ref(), cx));
                if index == self.active {
                    self.select(index, window, cx);
                } else {
                    self.sessions[index]
                        .terminal
                        .update(cx, |view, _| view.set_front(false, window));
                }
                log::info!("ssh: session {id:?} reconnected in place as {new_id:?}");
            }
            Err(error) => {
                log::error!("ssh: reconnect failed: {error:#}");
                self.sessions[index].recovery.error = Some(format!("{error:#}"));
                if automatic {
                    self.schedule_reconnect(id, Some(255), window, cx);
                }
            }
        }
        cx.notify();
    }

    pub(super) fn connection_status(&self, cx: &Context<Self>) -> Option<Div> {
        let session = self.sessions.get(self.active)?;
        session.remote.as_ref()?;
        if !session.disconnected {
            return None;
        }
        let label = if session.recovery.waiting {
            text(
                cx,
                "et.ssh.reconnect_wait",
                &[("attempt", &session.recovery.attempts.to_string())],
            )
        } else {
            text(cx, "et.ssh.disconnected", &[])
        };
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().colors.border)
                .text_size(cx.theme().text_size(TextSize::Sm))
                .text_color(cx.theme().colors.fg_muted)
                .child(Icon::new(IconName::WifiOff))
                .child(div().flex_1().min_w_0().child(label).children(
                    session.recovery.error.as_ref().map(|error| {
                        div()
                            .text_color(cx.theme().colors.danger)
                            .child(crate::sessions::one_line(error))
                    }),
                )),
        )
    }
}
