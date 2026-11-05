use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use ely_gpui_component::{
    forms::{InputEvent, TextInput},
    i18n::text,
};
use gpui::{AppContext, Context, Entity, Subscription, Task, Window};

use crate::{
    ssh::{
        files::{Listing, RemotePath},
        remote::Remote,
    },
    workspace::Workspace,
};

mod view;

pub(in crate::workspace) struct Choice {
    connection: Remote,
    path: Entity<TextInput>,
    listing: Option<Listing>,
    selected: Option<RemotePath>,
    busy: bool,
    error: Option<String>,
    generation: u64,
    cancel: Arc<AtomicBool>,
    reading: Task<()>,
    _input: Subscription,
}

impl Drop for Choice {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn resolve(text: &str, current: Option<&RemotePath>) -> anyhow::Result<Option<RemotePath>> {
    anyhow::ensure!(
        !text.as_bytes().contains(&0),
        "ssh folder chooser: path contains NUL"
    );
    if text.is_empty() || text == "~" {
        return Ok(None);
    }
    let mut path = if text.starts_with('/') || text.starts_with('~') {
        Vec::new()
    } else {
        current.map_or_else(
            || b"./".to_vec(),
            |path| {
                let mut root = path.0.clone();
                if !root.ends_with(b"/") {
                    root.push(b'/');
                }
                root
            },
        )
    };
    path.extend_from_slice(text.as_bytes());
    Ok(Some(RemotePath(path)))
}

impl Workspace {
    pub(in crate::workspace) fn choose_remote_folder(
        &mut self,
        connection: Remote,
        folder: Option<RemotePath>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path =
            cx.new(|cx| TextInput::new(window, cx).label(text(cx, "et.files.remote_path", &[])));
        let input = cx.subscribe(&path, |this, input, event, cx| {
            if *event == InputEvent::Submit {
                let text = input.read(cx).text().to_string();
                let current = this
                    .remote_choice
                    .as_ref()
                    .and_then(|choice| choice.listing.as_ref())
                    .map(|listing| &listing.path);
                match resolve(&text, current) {
                    Ok(folder) => this.read_remote_choice(folder, cx),
                    Err(error) => {
                        if let Some(choice) = &mut this.remote_choice {
                            choice.error = Some(error.to_string());
                        }
                        cx.notify();
                    }
                }
            }
        });
        log::info!("ssh: choosing folder on {}", connection.host.destination());
        self.choosing_folder = true;
        self.remote_choice = Some(Choice {
            connection,
            path,
            listing: None,
            selected: None,
            busy: false,
            error: None,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            reading: Task::ready(()),
            _input: input,
        });
        self.read_remote_choice(folder, cx);
    }

    fn read_remote_choice(&mut self, folder: Option<RemotePath>, cx: &mut Context<Self>) {
        let Some(choice) = &mut self.remote_choice else {
            return;
        };
        choice.cancel.store(true, Ordering::Relaxed);
        choice.cancel = Arc::new(AtomicBool::new(false));
        choice.generation += 1;
        choice.busy = true;
        choice.selected = None;
        choice.error = None;
        let (connection, generation, cancel) = (
            choice.connection.clone(),
            choice.generation,
            choice.cancel.clone(),
        );
        let ticket = cancel.clone();
        let task = cx.background_spawn(async move { connection.list(folder.as_ref(), &cancel) });
        choice.reading = cx.spawn(async move |workspace, cx| {
            let result = task.await;
            workspace
                .update(cx, |this, cx| {
                    let Some(choice) = &mut this.remote_choice else {
                        return;
                    };
                    if choice.generation != generation || !Arc::ptr_eq(&choice.cancel, &ticket) {
                        return;
                    }
                    choice.busy = false;
                    match result {
                        Ok(listing) => {
                            log::info!("ssh folder chooser: {}", listing.path.shown());
                            choice.path.update(cx, |input, cx| {
                                input.set_text(String::from_utf8_lossy(&listing.path.0), cx)
                            });
                            choice.listing = Some(listing);
                        }
                        Err(error) => {
                            log::error!("ssh folder chooser: {error:#}");
                            choice.error = Some(format!("{error:#}"));
                        }
                    }
                    cx.notify();
                })
                .ok();
        });
        cx.notify();
    }

    fn close_remote_choice(&mut self, cx: &mut Context<Self>) {
        self.remote_choice = None;
        self.choosing_folder = false;
        log::info!("ssh folder chooser: closed");
        cx.notify();
    }

    fn open_remote_choice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(choice) = &self.remote_choice else {
            return;
        };
        if choice.busy || choice.error.is_some() {
            return;
        }
        let Some(listing) = &choice.listing else {
            return;
        };
        let connection = choice.connection.clone();
        let folder = choice
            .selected
            .clone()
            .unwrap_or_else(|| listing.path.clone());
        let alive = self.sessions.iter().any(|session| {
            !session.disconnected
                && super::super::remote::endpoint(session).as_ref() == Some(&connection)
        });
        if !alive {
            self.remote_choice.as_mut().unwrap().error =
                Some("ssh folder chooser: source session disconnected or closed".into());
            return cx.notify();
        }
        self.close_remote_choice(cx);
        self.connect_at(connection.host, Some(folder), window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_choices_resolve_against_the_remote_folder_without_local_conversion() {
        let current = RemotePath(b"/srv/space ' directory".to_vec());
        assert_eq!(
            resolve("../next\\folder", Some(&current)).unwrap(),
            Some(RemotePath(
                b"/srv/space ' directory/../next\\folder".to_vec()
            ))
        );
        assert_eq!(
            resolve("~/project", Some(&current)).unwrap(),
            Some(RemotePath(b"~/project".to_vec()))
        );
        assert_eq!(
            resolve("/opt/project", Some(&current)).unwrap(),
            Some(RemotePath(b"/opt/project".to_vec()))
        );
        assert_eq!(resolve("~", Some(&current)).unwrap(), None);
        assert!(resolve("bad\0path", Some(&current)).is_err());
    }
}
