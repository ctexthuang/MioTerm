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
mod interaction;
mod menu;
mod plan;
mod refresh;
mod status;
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
    refreshing: bool,
    marks: status::Marks,
    git_error: Option<String>,
    marking: bool,
    checking: Option<Task<()>>,
    generation: usize,
    menu: Option<menu::Ask>,
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
    transfers: Entity<crate::ssh::transfers::Transfers>,
    show_transfers: bool,
    menu_number: u64,
    _transfers: Subscription,
    _finished: Subscription,
    transfer_error: Option<String>,
    front: bool,
    _polling: Task<()>,
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
        let transfers = crate::ssh::transfers::Transfers::get(cx);
        let observed = cx.observe(&transfers, |_, _, cx| cx.notify());
        let finished = cx.subscribe(
            &transfers,
            |panel, _, event: &crate::ssh::transfers::Finished, cx| {
                panel.refresh_local(cx);
                if panel
                    .connection
                    .as_ref()
                    .is_some_and(|connection| connection.session == event.0.session)
                    && !panel.disconnected
                {
                    panel.refresh_remote(cx);
                }
            },
        );
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
            transfers,
            show_transfers: false,
            menu_number: 0,
            _transfers: observed,
            _finished: finished,
            transfer_error: None,
            front: false,
            _polling: Self::poll(cx),
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

    fn key(&self, local: bool, entry: &Entry) -> SharedString {
        plan::key(
            self.connection.as_ref().map(|remote| remote.session),
            local,
            self.browser(local)
                .listing
                .as_ref()
                .map(|listing| &listing.path),
            entry,
        )
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
        self.front = true;
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
            self.remote.refreshing = false;
            self.remote.checking = None;
            self.remote.marking = false;
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

    pub fn hide(&mut self) {
        self.front = false;
    }

    pub fn refresh_remote(&mut self, cx: &mut Context<Self>) {
        self.navigate(false, self.remote.folder.clone(), cx);
    }

    fn navigate(&mut self, local: bool, folder: Option<RemotePath>, cx: &mut Context<Self>) {
        self.load(local, folder, false, cx);
    }

    fn load(
        &mut self,
        local: bool,
        folder: Option<RemotePath>,
        background: bool,
        cx: &mut Context<Self>,
    ) {
        if !local && self.disconnected {
            return;
        }
        let connection = self.connection.clone();
        let home = self.home.clone();
        let browser = self.browser_mut(local);
        browser.cancel = Cancel::default();
        let cancel = browser.cancel.0.clone();
        let ticket = cancel.clone();
        if browser.folder != folder {
            browser.listing = None;
            browser.selected.clear();
            browser.marks = status::Marks::default();
            browser.git_error = None;
            browser.checking = None;
            browser.marking = false;
            browser.generation += 1;
        }
        browser.folder = folder.clone();
        if !background {
            browser.error = None;
        }
        browser.busy = !background;
        browser.refreshing = background;
        let generation = browser.generation;
        let source = connection.clone();
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
                .update(cx, |panel, cx| {
                    if panel.browser(local).generation != generation
                        || !Arc::ptr_eq(&panel.browser(local).cancel.0, &ticket)
                        || (!local && panel.connection != source)
                    {
                        return;
                    }
                    panel.store(local, result, background, cx);
                })
                .ok();
        }));
        if !background {
            cx.notify();
        }
    }

    fn store(
        &mut self,
        local: bool,
        result: anyhow::Result<Listing>,
        background: bool,
        cx: &mut Context<Self>,
    ) {
        let session = self.connection.as_ref().map(|remote| remote.session);
        let browser = self.browser_mut(local);
        browser.busy = false;
        browser.refreshing = false;
        match result {
            Ok(listing) => {
                if background
                    && browser.listing.as_ref() == Some(&listing)
                    && browser.error.is_none()
                {
                    return;
                }
                log::debug!(
                    "{} files: refreshed {}",
                    if local { "local" } else { "remote" },
                    listing.path.shown()
                );
                if browser
                    .listing
                    .as_ref()
                    .is_some_and(|old| old.path != listing.path)
                {
                    browser.checking = None;
                    browser.marking = false;
                    browser.marks = status::Marks::default();
                    browser.git_error = None;
                }
                browser.error = None;
                browser.folder = Some(listing.path.clone());
                let keys = listing
                    .entries
                    .iter()
                    .map(|entry| plan::key(session, local, Some(&listing.path), entry))
                    .collect::<std::collections::HashSet<_>>();
                browser.selected.retain(|key| keys.contains(key));
                browser.listing = Some(listing);
                let path = browser.folder.clone().expect("a listed folder");
                let input = if local {
                    &self.local_path
                } else {
                    &self.remote_path
                };
                if !background {
                    input.update(cx, |input, cx| {
                        input.set_text(String::from_utf8_lossy(&path.0), cx)
                    });
                }
                if !local
                    && !background
                    && let Some(connection) = &self.connection
                {
                    cx.emit(FolderChanged {
                        connection: connection.clone(),
                        folder: path,
                    });
                }
                self.check_marks(local, cx);
            }
            Err(error) => {
                let error = format!("{error:#}");
                if browser.error.as_ref() == Some(&error) {
                    return;
                }
                log::error!(
                    "{} files: {error:#}",
                    if local { "local" } else { "remote" }
                );
                browser.error = Some(error);
            }
        }
        cx.notify();
    }
}
