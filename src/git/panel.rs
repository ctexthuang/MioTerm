use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use ely_gpui_component::{
    forms::TextInput,
    git::{ChangeAction, ChangeSection, Commit, DiffLayout, Stretch},
    i18n::{I18n, text},
};
use gpui::{AppContext, Context, Entity, EventEmitter, SharedString, Subscription, Task, Window};

use self::view::{is_folder, shown};
use super::{Backend, Change, Repo};

mod view;

/// The newest commits the panel lists.
const RECENT: usize = 50;

/// What the shown folder reads as.
#[derive(PartialEq)]
enum State {
    Reading,
    Outside,
    Failed(SharedString),
    Read(Repo, Rc<Vec<Commit>>),
}

/// An open diff: file, section, lines or why none.
struct Diff {
    change: Change,
    section: ChangeSection,
    lines: Result<Rc<Vec<Stretch>>, &'static str>,
}

/// The panel changed the repository, so other views read it again.
pub struct Acted;

/// The changes `section` lists.
fn listed(repo: &Repo, section: ChangeSection) -> &[Change] {
    match section {
        ChangeSection::Conflicted => &repo.conflicted,
        ChangeSection::Staged => &repo.staged,
        ChangeSection::Unstaged => &repo.unstaged,
        ChangeSection::Untracked => &repo.untracked,
        ChangeSection::Ignored => &[],
    }
}

/// The Git panel: the shown folder's repository, its changes, a commit, recent commits and diffs.
pub struct GitPanel {
    backend: Backend,
    enabled: bool,
    folder: PathBuf,
    state: State,
    /// Where `state` was read: an action waits until it is `folder`.
    read: PathBuf,
    /// The last action's failure: what failed, and git's words.
    error: Option<(&'static str, SharedString)>,
    subject: Entity<TextInput>,
    body: Entity<TextInput>,
    amend: bool,
    diff: Option<Diff>,
    layout: DiffLayout,
    discarding: Option<(Change, ChangeSection)>,
    reading: Task<()>,
    opening: Task<()>,
    /// The last action asked; each waits for the one before it.
    acting: Task<()>,
    _language: Subscription,
}

impl EventEmitter<Acted> for GitPanel {}

impl GitPanel {
    pub fn unavailable(&mut self, backend: Backend, message: &str, cx: &mut Context<Self>) {
        if self.backend != backend {
            self.folder = PathBuf::from(".");
        }
        self.backend = backend;
        self.enabled = false;
        self.error = None;
        self.state = State::Failed(message.to_string().into());
        self.reading = Task::ready(());
        self.opening = Task::ready(());
        self.diff = None;
        self.discarding = None;
        cx.notify();
    }

    pub fn new(folder: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (summary, description) = (
            text(cx, "et.git.summary", &[]),
            text(cx, "et.git.description", &[]),
        );
        let subject = cx.new(|cx| TextInput::new(window, cx).placeholder(summary));
        let body = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(3, 6)
                .placeholder(description)
        });
        Self {
            backend: Backend::Local,
            enabled: true,
            read: folder.clone(),
            folder,
            state: State::Reading,
            error: None,
            subject,
            body,
            amend: false,
            diff: None,
            layout: DiffLayout::Unified,
            discarding: None,
            reading: Task::ready(()),
            opening: Task::ready(()),
            acting: Task::ready(()),
            _language: cx.observe_global::<I18n>(Self::speak),
        }
    }

    /// Words its fields again in the language just chosen.
    fn speak(&mut self, cx: &mut Context<Self>) {
        let (subject, body) = (self.subject.clone(), self.body.clone());
        subject.update(cx, |input, cx| {
            input.set_placeholder(text(cx, "et.git.summary", &[]), cx)
        });
        body.update(cx, |input, cx| {
            input.set_placeholder(text(cx, "et.git.description", &[]), cx)
        });
    }

    /// Shows `folder`'s repository, read afresh; a new folder drops what the old one had open.
    pub fn show(&mut self, folder: &Path, cx: &mut Context<Self>) {
        self.show_on(Backend::Local, folder, cx);
    }

    pub fn show_on(&mut self, backend: Backend, folder: &Path, cx: &mut Context<Self>) {
        self.enabled = true;
        if !backend.same_path(folder, &self.folder) || backend != self.backend {
            log::info!("git panel: shows {}", folder.display());
            // Inside the shown work tree the view stays while it reads, so a cd does not blank it.
            let inside = backend == self.backend
                && matches!(&self.state, State::Read(repo, _) if backend.contains(folder, &repo.top));
            if !inside {
                self.state = State::Reading;
            }
            self.folder = folder.to_path_buf();
            self.backend = backend;
            (self.error, self.diff, self.discarding) = (None, None, None);
            self.opening = Task::ready(());
            cx.notify();
        }
        self.read(cx);
    }

    /// The repository shown, once it was read in the shown folder.
    fn current(&self) -> Option<&Repo> {
        match &self.state {
            State::Read(repo, _) if self.backend.same_path(&self.read, &self.folder) => Some(repo),
            _ => None,
        }
    }

    fn waits(&self, asked: &str) {
        let folder = self.folder.display();
        log::info!("git panel: {asked} waits for {folder} to read; nothing done");
    }

    /// Reads the repository and its recent commits, off the main thread.
    fn read(&mut self, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let folder = self.folder.clone();
        let backend = self.backend.clone();
        let read = cx.background_spawn(async move {
            let Some(repo) = backend.repo(&folder)? else {
                return Ok(None);
            };
            // Git refuses a log before the first commit.
            let commits = match repo.commit {
                Some(_) => backend.log(&repo.top, RECENT)?,
                None => Vec::new(),
            };
            anyhow::Ok(Some((repo, commits)))
        });
        self.reading = cx.spawn(async move |panel, cx| {
            let read = read.await;
            panel.update(cx, |panel, cx| panel.store(read, cx)).ok();
        });
    }

    fn store(&mut self, read: anyhow::Result<Option<(Repo, Vec<Commit>)>>, cx: &mut Context<Self>) {
        let state = match read {
            Ok(Some((repo, commits))) => State::Read(repo, Rc::new(commits)),
            Ok(None) => State::Outside,
            Err(error) => {
                let folder = self.folder.display();
                log::error!("git panel: {folder} did not read: {error:#}");
                State::Failed(format!("{error:#}").into())
            }
        };
        self.read = self.folder.clone();
        if state == self.state {
            return;
        }
        match &state {
            State::Read(repo, commits) => log::debug!(
                "git panel: {} on {:?}, {} conflicted, {} staged, {} unstaged, {} untracked, {} commits",
                repo.top.display(),
                repo.branch,
                repo.conflicted.len(),
                repo.staged.len(),
                repo.unstaged.len(),
                repo.untracked.len(),
                commits.len()
            ),
            State::Outside => log::debug!("git panel: {} is no repository", self.folder.display()),
            State::Reading | State::Failed(_) => {}
        }
        self.state = state;
        cx.notify();
    }

    /// Runs `work` on the repository at `top` off the main thread once the action before it is done; on success `then`; after it, a fresh read and word to other views.
    fn act(
        &mut self,
        what: &'static str,
        top: PathBuf,
        work: impl FnOnce(&Backend, &Path) -> anyhow::Result<()> + Send + 'static,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        let backend = self.backend.clone();
        let source = backend.clone();
        let before = std::mem::replace(&mut self.acting, Task::ready(()));
        self.acting = cx.spawn(async move |panel, cx| {
            before.await;
            let done = cx
                .background_spawn(async move { work(&backend, &top) })
                .await;
            let acted = |panel: &mut Self, cx: &mut Context<Self>| {
                if panel.backend != source {
                    if let Err(error) = done {
                        log::error!("git panel: {what} on previous connection: {error:#}");
                    }
                    return;
                }
                match done {
                    Ok(()) => then(panel, cx),
                    Err(error) => {
                        log::error!("git panel: {what}: {error:#}");
                        panel.error = Some((what, format!("{error:#}").into()));
                    }
                }
                panel.read(cx);
                cx.emit(Acted);
                cx.notify();
            };
            panel.update(cx, acted).ok();
        });
        cx.notify();
    }

    /// What a row asked: `row` names its file in `section`.
    fn row_action(
        &mut self,
        row: &SharedString,
        section: ChangeSection,
        action: ChangeAction,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.current() else {
            return self.waits(&format!("{action:?} {row}"));
        };
        let found = listed(repo, section)
            .iter()
            .find(|change| shown(&change.path) == *row);
        let (Some(change), top) = (found.cloned(), repo.top.clone()) else {
            log::warn!("git panel: {row} is no longer in {section:?}");
            return;
        };
        log::debug!("git panel: {action:?} {row} in {section:?}");
        match action {
            ChangeAction::Stage => {
                let work = move |backend: &Backend, top: &Path| backend.stage(top, &[change]);
                self.act("et.git.stage_failed", top, work, |_, _| {}, cx)
            }
            ChangeAction::Unstage => {
                let work = move |backend: &Backend, top: &Path| backend.unstage(top, &[change]);
                self.act("et.git.unstage_failed", top, work, |_, _| {}, cx)
            }
            ChangeAction::Discard => {
                self.discarding = Some((change, section));
                cx.notify();
            }
            ChangeAction::Open => self.open(top, change, section, cx),
        }
    }

    /// Stages every unstaged change, or unstages every staged one.
    fn all(&mut self, stage: bool, cx: &mut Context<Self>) {
        let Some(repo) = self.current() else {
            return self.waits(if stage { "Stage all" } else { "Unstage all" });
        };
        let top = repo.top.clone();
        if stage {
            let changes = repo
                .unstaged
                .iter()
                .chain(&repo.untracked)
                .cloned()
                .collect::<Vec<_>>();
            let work = move |backend: &Backend, top: &Path| backend.stage(top, &changes);
            self.act("et.git.stage_failed", top, work, |_, _| {}, cx);
        } else {
            let changes = repo.staged.clone();
            let work = move |backend: &Backend, top: &Path| backend.unstage(top, &changes);
            self.act("et.git.unstage_failed", top, work, |_, _| {}, cx);
        }
    }

    fn discard(&mut self, change: Change, section: ChangeSection, cx: &mut Context<Self>) {
        let Some(repo) = self.current() else {
            return self.waits(&format!("Discard {}", change.path.display()));
        };
        let top = repo.top.clone();
        log::info!("git panel: discarding {} confirmed", change.path.display());
        let work = move |backend: &Backend, top: &Path| backend.discard(top, &change, section);
        self.act("et.git.discard_failed", top, work, |_, _| {}, cx);
    }

    /// Commits what is staged with the typed message; on success the fields that still hold it empty.
    fn commit(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.current() else {
            return self.waits("Commit");
        };
        let top = repo.top.clone();
        let subject = self.subject.read(cx).text().to_string();
        let body = self.body.read(cx).text().to_string();
        let message = match body.trim() {
            "" => subject.trim().to_string(),
            body => format!("{}\n\n{body}", subject.trim()),
        };
        let amend = self.amend;
        let work = move |backend: &Backend, top: &Path| backend.commit(top, &message, amend);
        let committed = top.clone();
        // A draft typed, or a box ticked, while it ran stays, as does all while a folder reads or in another repository.
        let then = move |panel: &mut Self, cx: &mut Context<Self>| {
            if panel
                .current()
                .is_none_or(|repo| !panel.backend.same_path(&repo.top, &committed))
            {
                return;
            }
            for (input, text) in [(&panel.subject, subject), (&panel.body, body)] {
                if input.read(cx).text() == text {
                    input.update(cx, |input, cx| input.set_text("", cx));
                }
            }
            if panel.amend == amend {
                panel.amend = false;
            }
        };
        self.act("et.git.commit_failed", top, work, then, cx);
    }

    /// Opens `change`'s diff as `section` holds it, once git gives it.
    fn open(
        &mut self,
        top: PathBuf,
        change: Change,
        section: ChangeSection,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        // Git keeps an untracked folder whole, so it has no lines.
        if section == ChangeSection::Untracked && is_folder(&change.path) {
            self.opening = Task::ready(());
            let lines = Err("et.git.untracked_folder");
            return self.shows(
                Diff {
                    change,
                    section,
                    lines,
                },
                cx,
            );
        }
        let asked = change.clone();
        let backend = self.backend.clone();
        let read = cx.background_spawn(async move { backend.diff(&top, &asked, section) });
        self.opening = cx.spawn(async move |panel, cx| {
            let read = read.await;
            let opened = |panel: &mut Self, cx: &mut Context<Self>| {
                let lines = match read {
                    Ok(Some(lines)) if lines.is_empty() => Err("et.git.no_line"),
                    Ok(Some(lines)) => Ok(Rc::new(lines)),
                    Ok(None) => Err("et.git.binary"),
                    Err(error) => {
                        log::error!("git panel: the diff failed: {error:#}");
                        panel.error = Some(("et.git.diff_failed", format!("{error:#}").into()));
                        return cx.notify();
                    }
                };
                panel.shows(
                    Diff {
                        change,
                        section,
                        lines,
                    },
                    cx,
                )
            };
            panel.update(cx, opened).ok();
        });
    }

    fn shows(&mut self, diff: Diff, cx: &mut Context<Self>) {
        let (path, section) = (diff.change.path.display(), diff.section);
        log::info!("git panel: shows the diff of {path} in {section:?}");
        self.diff = Some(diff);
        cx.notify();
    }
}
