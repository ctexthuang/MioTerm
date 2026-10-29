use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{AppContext, Context, Entity, EventEmitter, SharedString, Subscription, Task, Window};

use super::{Entry, Listing, Remote, RemotePath, Transfer};

mod actions;
mod confirm;
mod upload;
mod view;

pub struct FolderChanged {
    pub connection: Remote,
    pub folder: RemotePath,
}

struct Cancel(Arc<AtomicBool>);

impl Default for Cancel {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Browser {
    folder: Option<RemotePath>,
    listing: Option<Listing>,
    selected: Vec<SharedString>,
    error: Option<String>,
    busy: bool,
    generation: usize,
    cancel: Cancel,
    reading: Option<Task<()>>,
}

pub struct RemoteFiles {
    connection: Option<Remote>,
    disconnected: bool,
    local: Browser,
    remote: Browser,
    home: PathBuf,
    local_path: Entity<TextInput>,
    remote_path: Entity<TextInput>,
    pending: Option<Transfer>,
    transfer: Option<Cancel>,
    transferring: Option<Task<()>>,
    transfer_status: Option<String>,
    transfer_error: Option<String>,
    _inputs: Vec<Subscription>,
}

impl EventEmitter<FolderChanged> for RemoteFiles {}

fn local_list(path: &std::path::Path) -> anyhow::Result<Listing> {
    use anyhow::{Context as _, ensure};
    #[cfg(not(windows))]
    let path = std::fs::canonicalize(path).context("local files: cannot open folder")?;
    #[cfg(windows)]
    let path = std::path::absolute(path).context("local files: cannot resolve folder")?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&path).context("local files: cannot list folder")? {
        ensure!(
            entries.len() < 10000,
            "local files: directory exceeds 10000 entries"
        );
        let entry = entry?;
        let kind = entry.file_type()?;
        let info = entry.metadata()?;
        entries.push(Entry {
            name: RemotePath(entry.file_name().as_encoded_bytes().to_vec()),
            directory: kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()),
            regular: kind.is_file(),
            link: kind.is_symlink(),
            size: info.len(),
        });
    }
    entries.sort_by_cached_key(|entry| {
        (
            !entry.directory,
            entry.name.shown().to_lowercase(),
            entry.name.0.clone(),
        )
    });
    Ok(Listing {
        parent: RemotePath::from_path(path.parent().unwrap_or(&path)),
        path: RemotePath::from_path(&path),
        entries,
    })
}

impl RemoteFiles {
    pub fn new(home: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let local_path = cx.new(|cx| TextInput::new(window, cx).label("Local path"));
        let remote_path = cx.new(|cx| TextInput::new(window, cx).label("Remote path"));
        let inputs = [(&local_path, true), (&remote_path, false)]
            .into_iter()
            .map(|(input, local)| {
                cx.subscribe(input, move |panel, input, event, cx| {
                    if *event == InputEvent::Submit {
                        let path = input.read(cx).text().to_string();
                        panel.enter(local, path, cx);
                    }
                })
            })
            .collect();
        Self {
            connection: None,
            disconnected: false,
            local: Browser {
                folder: Some(RemotePath::from_path(&home)),
                ..Browser::default()
            },
            remote: Browser::default(),
            home,
            local_path,
            remote_path,
            pending: None,
            transfer: None,
            transferring: None,
            transfer_status: None,
            transfer_error: None,
            _inputs: inputs,
        }
    }

    fn browser(&self, local: bool) -> &Browser {
        if local { &self.local } else { &self.remote }
    }

    fn browser_mut(&mut self, local: bool) -> &mut Browser {
        if local {
            &mut self.local
        } else {
            &mut self.remote
        }
    }

    fn enter(&mut self, local: bool, text: String, cx: &mut Context<Self>) {
        let current = self.browser(local).folder.clone();
        if current
            .as_ref()
            .is_some_and(|path| String::from_utf8_lossy(&path.0) == text)
        {
            return self.navigate(local, current, cx);
        }
        if text.is_empty() || text == "~" {
            return self.navigate(local, None, cx);
        }
        let path = if local {
            let path = std::path::PathBuf::from(&text);
            let path = if let Some(rest) = text.strip_prefix("~/") {
                Ok(self.home.join(rest))
            } else if path.is_absolute() {
                Ok(path)
            } else {
                current
                    .as_ref()
                    .map_or_else(|| Ok(self.home.clone()), RemotePath::local_path)
                    .map(|folder| folder.join(path))
            };
            path.map(|path| RemotePath::from_path(&path))
        } else if text.starts_with('/') || text.starts_with('~') {
            Ok(RemotePath(text.into_bytes()))
        } else {
            let mut path = current.map_or_else(|| b"./".to_vec(), |path| path.0);
            if !path.ends_with(b"/") {
                path.push(b'/');
            }
            path.extend_from_slice(text.as_bytes());
            Ok(RemotePath(path))
        };
        match path {
            Ok(path) => self.navigate(local, Some(path), cx),
            Err(error) => {
                self.browser_mut(local).error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }

    pub fn show(
        &mut self,
        connection: Remote,
        folder: Option<RemotePath>,
        disconnected: bool,
        cx: &mut Context<Self>,
    ) {
        let changed = self.connection.as_ref() != Some(&connection);
        let ended = self.disconnected != disconnected;
        if changed {
            self.remote = Browser::default();
            self.pending = None;
            self.connection = Some(connection);
        }
        self.disconnected = disconnected;
        if disconnected {
            self.remote.cancel = Cancel::default();
            self.remote.reading = None;
            self.remote.busy = false;
            self.remote.error = Some("SSH session disconnected.".into());
        } else if changed || ended || folder != self.remote.folder || self.remote.listing.is_none()
        {
            self.navigate(false, folder, cx);
        }
        if self.local.listing.is_none() && !self.local.busy {
            self.refresh_local(cx);
        }
        cx.notify();
    }

    pub fn refresh_local(&mut self, cx: &mut Context<Self>) {
        self.navigate(true, self.local.folder.clone(), cx);
    }

    pub fn refresh_remote(&mut self, cx: &mut Context<Self>) {
        self.navigate(false, self.remote.folder.clone(), cx);
    }

    fn navigate(&mut self, local: bool, folder: Option<RemotePath>, cx: &mut Context<Self>) {
        if !local && self.disconnected {
            return;
        }
        let connection = self.connection.clone();
        let home = self.home.clone();
        let browser = self.browser_mut(local);
        browser.cancel = Cancel::default();
        let cancel = browser.cancel.0.clone();
        if browser.folder != folder {
            browser.listing = None;
            browser.selected.clear();
            browser.generation += 1;
        }
        browser.folder = folder.clone();
        browser.error = None;
        browser.busy = true;
        let read = cx.background_spawn(async move {
            if local {
                let path = match folder {
                    Some(path) if path.0 == b"~" || path.0.is_empty() => home,
                    Some(path) => path.local_path()?,
                    None => home,
                };
                local_list(&path)
            } else {
                let connection =
                    connection.ok_or_else(|| anyhow::anyhow!("remote files: no SSH connection"))?;
                connection.list(folder.as_ref(), &cancel)
            }
        });
        self.browser_mut(local).reading = Some(cx.spawn(async move |panel, cx| {
            let result = read.await;
            panel
                .update(cx, |panel, cx| panel.store(local, result, cx))
                .ok();
        }));
        cx.notify();
    }

    fn store(&mut self, local: bool, result: anyhow::Result<Listing>, cx: &mut Context<Self>) {
        let browser = self.browser_mut(local);
        browser.busy = false;
        match result {
            Ok(listing) => {
                log::info!(
                    "{} files: folder {}",
                    if local { "local" } else { "remote" },
                    listing.path.shown()
                );
                browser.folder = Some(listing.path.clone());
                let keys = listing.entries.iter().map(view::key).collect::<Vec<_>>();
                browser.selected.retain(|key| keys.contains(key));
                browser.listing = Some(listing);
                let path = browser.folder.clone().expect("a listed folder");
                let input = if local {
                    &self.local_path
                } else {
                    &self.remote_path
                };
                input.update(cx, |input, cx| {
                    input.set_text(String::from_utf8_lossy(&path.0), cx)
                });
                if !local && let Some(connection) = &self.connection {
                    cx.emit(FolderChanged {
                        connection: connection.clone(),
                        folder: path,
                    });
                }
            }
            Err(error) => {
                log::error!(
                    "{} files: {error:#}",
                    if local { "local" } else { "remote" }
                );
                browser.error = Some(format!("{error:#}"));
            }
        }
        cx.notify();
    }
}
