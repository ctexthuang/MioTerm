use gpui::Context;

use super::Workspace;
use crate::{
    git::Backend,
    sessions::Session,
    ssh::{
        files::{RemotePath, panel::FolderChanged},
        remote::Remote,
    },
};

fn endpoint(session: &Session) -> Option<Remote> {
    Some(Remote {
        session: session.terminal.entity_id().as_u64(),
        host: session.remote.clone()?,
        socket: session
            .control
            .as_ref()
            .and_then(|control| control.socket()),
    })
}

impl Workspace {
    pub(super) fn follow_files(&mut self, cx: &mut Context<Self>) {
        let session = &self.sessions[self.active];
        if let Some(connection) = endpoint(session) {
            let folder = session.remote_folder.clone();
            let disconnected = session.disconnected;
            self.remote_files.update(cx, |files, cx| {
                files.show(connection.clone(), folder.clone(), disconnected, cx)
            });
            if disconnected {
                self.git.update(cx, |git, cx| {
                    git.unavailable(Backend::Remote(connection), "SSH session disconnected.", cx)
                });
            } else if let Some(folder) = folder {
                match folder.local_path() {
                    Ok(folder) => self.git.update(cx, |git, cx| {
                        git.show_on(Backend::Remote(connection), &folder, cx)
                    }),
                    Err(error) => self.git.update(cx, |git, cx| {
                        git.unavailable(Backend::Remote(connection), &format!("{error:#}"), cx)
                    }),
                }
            } else {
                self.git.update(cx, |git, cx| {
                    git.unavailable(
                        Backend::Remote(connection),
                        "Waiting for the remote folder. Refresh Files after signing in.",
                        cx,
                    )
                });
            }
        } else {
            let folder = session.folder.clone();
            self.files.update(cx, |files, cx| files.show(&folder, cx));
            self.git.update(cx, |git, cx| git.show(&folder, cx));
        }
    }

    pub(super) fn remote_folder(&mut self, event: &FolderChanged, cx: &mut Context<Self>) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|session| endpoint(session).as_ref() == Some(&event.connection))
        else {
            return;
        };
        self.sessions[index].remote_folder = Some(event.folder.clone());
        if index == self.active {
            match event.folder.local_path() {
                Ok(folder) => self.git.update(cx, |git, cx| {
                    git.show_on(Backend::Remote(event.connection.clone()), &folder, cx)
                }),
                Err(error) => self.git.update(cx, |git, cx| {
                    git.unavailable(
                        Backend::Remote(event.connection.clone()),
                        &format!("{error:#}"),
                        cx,
                    )
                }),
            }
            cx.notify();
        }
    }

    pub(super) fn reported_remote_folder(
        &mut self,
        index: usize,
        folder: &std::path::Path,
        cx: &mut Context<Self>,
    ) {
        self.sessions[index].remote_folder = Some(RemotePath::from_path(folder));
        if index == self.active {
            self.follow_files(cx);
        }
    }
}
