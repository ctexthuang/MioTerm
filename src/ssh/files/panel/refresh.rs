use std::time::Duration;

use gpui::{AppContext, Context, Task};

use super::RemoteFiles;
use crate::{
    git::Backend,
    settings::{PanelTab, SettingsStore},
};

impl RemoteFiles {
    pub(super) fn poll(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |panel, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if panel.update(cx, |panel, cx| panel.tick(cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let settings = SettingsStore::get(cx);
        if !self.front
            || !settings.show_panel
            || settings.panel != PanelTab::Files
            || self.disconnected
            || self.transfer.is_some()
            || self.pending.is_some()
        {
            return;
        }
        for local in [true, false] {
            let browser = self.browser(local);
            if !browser.busy && !browser.refreshing {
                self.load(local, browser.folder.clone(), true, cx);
            }
            self.check_marks(local, cx);
        }
    }

    pub(super) fn check_marks(&mut self, local: bool, cx: &mut Context<Self>) {
        let browser = self.browser(local);
        if browser.marking
            || browser.busy
            || browser.error.is_some()
            || (!local && self.disconnected)
        {
            return;
        }
        let Some(listing) = &browser.listing else {
            return;
        };
        let root = listing.path.clone();
        let folder = root.clone();
        let generation = browser.generation;
        let source = self.connection.clone();
        let backend = if local {
            Backend::Local
        } else if let Some(connection) = &source {
            Backend::Remote(connection.clone())
        } else {
            return;
        };
        self.browser_mut(local).marking = true;
        let read = cx.background_spawn(async move { super::status::read(&backend, &root) });
        self.browser_mut(local).checking = Some(cx.spawn(async move |panel, cx| {
            let result = read.await;
            panel
                .update(cx, |panel, cx| {
                    let connected = local || panel.connection == source;
                    let browser = panel.browser_mut(local);
                    if !connected
                        || browser.generation != generation
                        || browser.listing.as_ref().map(|listing| &listing.path) != Some(&folder)
                    {
                        return;
                    }
                    browser.marking = false;
                    let changed = match result {
                        Ok(marks) => {
                            let recovered = browser.git_error.take().is_some();
                            if recovered {
                                log::info!("files: Git status recovered");
                            }
                            let changed = recovered || marks != browser.marks;
                            browser.marks = marks;
                            changed
                        }
                        Err(error) => {
                            let changed = browser.git_error.as_ref() != Some(&error);
                            if changed {
                                log::error!("files: Git status failed: {error}");
                            }
                            browser.git_error = Some(error);
                            changed
                        }
                    };
                    if changed {
                        cx.notify();
                    }
                })
                .ok();
        }));
    }
}
