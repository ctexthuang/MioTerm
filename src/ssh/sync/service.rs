use std::time::{Duration, Instant};

use anyhow::ensure;
use ely_gpui_component::primitives::Severity;
use gpui::{App, Global};

use super::{
    Request, Restored,
    saved::{Profile, Secrets, VaultStore},
};
use crate::{
    settings::SettingsStore,
    ssh::{
        Host, HostStore,
        vault::{Vault, migration},
    },
};

mod queue;
use queue::Queue;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Operation {
    Save,
    Sync,
    Restore,
}

pub(super) struct SyncService {
    pub busy: bool,
    pub needs_import: bool,
    pub status: Option<(Severity, String)>,
    queue: Queue,
    hosts: Vec<Host>,
    profile: Profile,
}

impl Global for SyncService {}

pub(super) fn profile(cx: &App) -> Profile {
    let saved = SettingsStore::get(cx);
    Profile {
        webdav: saved.webdav.clone(),
        settings: saved.sync.clone(),
    }
}

pub fn start(cx: &mut App) {
    cx.set_global(SyncService {
        busy: false,
        needs_import: false,
        status: None,
        queue: Queue::default(),
        hosts: HostStore::get(cx).hosts.clone(),
        profile: profile(cx),
    });
    cx.observe_global::<HostStore>(|cx| {
        let hosts = HostStore::get(cx).hosts.clone();
        if cx.global::<SyncService>().hosts != hosts {
            cx.global_mut::<SyncService>().hosts = hosts;
            changed(cx);
        }
    })
    .detach();
    cx.observe_global::<SettingsStore>(|cx| {
        let now = profile(cx);
        if cx.global::<SyncService>().profile != now {
            cx.global_mut::<SyncService>().profile = now;
            changed(cx);
        }
    })
    .detach();
    changed(cx);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            cx.update(|cx| {
                let service = cx.global::<SyncService>();
                if !service.busy
                    && service.profile.settings.auto_sync
                    && service.queue.ready(Instant::now())
                {
                    cx.global_mut::<SyncService>().queue.started();
                    let result = launch(profile(cx), Secrets::default(), Operation::Sync, true, cx);
                    if let Err(error) = result {
                        finish(Err(error), true, cx);
                    }
                }
            });
        }
    })
    .detach();
}

pub(crate) fn changed(cx: &mut App) {
    if cx.try_global::<SyncService>().is_some() && SettingsStore::get(cx).sync.auto_sync {
        cx.global_mut::<SyncService>().queue.changed(Instant::now());
        log::info!("SSH sync: local changes queued for automatic synchronization");
    }
}

pub(super) fn submit(
    profile: Profile,
    secrets: Secrets,
    operation: Operation,
    cx: &mut App,
) -> anyhow::Result<()> {
    launch(profile, secrets, operation, false, cx)
}

pub(super) fn import_native(cx: &mut App) -> anyhow::Result<()> {
    ensure!(
        !cx.global::<SyncService>().busy,
        "SSH sync: another synchronization operation is in progress"
    );
    let hosts = HostStore::get(cx)
        .hosts
        .iter()
        .chain(HostStore::get(cx).recent.iter().map(|recent| &recent.host))
        .cloned()
        .collect::<Vec<_>>();
    let token = SettingsStore::get(cx).sync.remember_token;
    let service = cx.global_mut::<SyncService>();
    service.busy = true;
    service.queue.started();
    service.status = Some((
        Severity::Info,
        "Importing system credentials into the application vault...".into(),
    ));
    log::info!("Credential vault: user requested system credential import");
    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { migration::run(&Vault::open()?, &hosts, token) })
            .await;
        cx.update(|cx| match result {
            Ok(report) => {
                let complete = report.missing.is_empty();
                let text = report.message();
                if complete {
                    changed(cx);
                }
                finish(Ok(text.clone()), false, cx);
                if !complete {
                    let service = cx.global_mut::<SyncService>();
                    service.needs_import = true;
                    service.status = Some((Severity::Warning, text));
                }
            }
            Err(error) => {
                finish(Err(error), false, cx);
                cx.global_mut::<SyncService>().needs_import = true;
            }
        });
    })
    .detach();
    Ok(())
}

fn launch(
    next: Profile,
    secrets: Secrets,
    operation: Operation,
    automatic: bool,
    cx: &mut App,
) -> anyhow::Result<()> {
    ensure!(
        !cx.global::<SyncService>().busy,
        "SSH sync: another synchronization operation is in progress"
    );
    next.webdav.validate()?;
    next.settings.validate(&next.webdav)?;
    let previous = profile(cx);
    let service = cx.global_mut::<SyncService>();
    service.busy = true;
    if operation == Operation::Sync {
        service.queue.started();
    }
    service.status = Some((
        Severity::Info,
        match operation {
            Operation::Save => "Saving synchronization settings...",
            Operation::Sync if automatic => "Automatically synchronizing encrypted SSH archive...",
            Operation::Sync => "Synchronizing encrypted SSH archive...",
            Operation::Restore => "Restoring encrypted SSH archive...",
        }
        .into(),
    ));
    log::info!("SSH sync: operation started; automatic: {automatic}");
    let target = next.clone();
    let before = previous.clone();
    let prepared = cx.background_executor().spawn(async move {
        let store = VaultStore(Vault::open()?);
        let secrets = target.resolve(secrets, operation != Operation::Save, &store)?;
        if !automatic {
            target.remember(&before, &secrets, &store)?;
        }
        Ok::<_, anyhow::Error>(secrets)
    });
    cx.spawn(async move |cx| {
        let prepared = prepared.await;
        let request = cx.update(|cx| -> anyhow::Result<Option<Request>> {
            let secrets = prepared?;
            ensure!(
                profile(cx) == previous,
                "SSH sync: synchronization settings changed during the operation; retry"
            );
            if !automatic {
                SettingsStore::update(cx, |saved| {
                    saved.webdav = next.webdav.clone();
                    saved.sync = next.settings.clone();
                })?;
                cx.global_mut::<SyncService>().profile = next.clone();
                if operation == Operation::Save {
                    changed(cx);
                }
            }
            if operation == Operation::Save {
                return Ok(None);
            }
            Ok(Some(Request {
                settings: next.webdav.clone(),
                gitee: (next.settings.provider == super::Provider::Gitee)
                    .then_some(next.settings.gitee.clone()),
                include_credentials: next.settings.include_credentials,
                remember_token: false,
                password: secrets.password,
                passphrase: secrets.passphrase,
                local: HostStore::get(cx).clone(),
                key_folder: crate::ssh::keys::folder()?,
                restore_only: operation == Operation::Restore,
            }))
        });
        let result = match request {
            Ok(Some(request)) => cx
                .background_executor()
                .spawn(async move { super::run(request) })
                .await
                .map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        cx.update(|cx| {
            let result = result.and_then(|restored| {
                ensure!(
                    profile(cx) == next || operation == Operation::Save,
                    "SSH sync: settings changed; remote results were not applied"
                );
                apply(restored, automatic, cx)
            });
            finish(result, automatic, cx);
        });
    })
    .detach();
    Ok(())
}

fn apply(restored: Option<Restored>, automatic: bool, cx: &mut App) -> anyhow::Result<String> {
    let Some(restored) = restored else {
        return Ok("Synchronization settings saved.".into());
    };
    let (hosts, keys, credentials) = (restored.hosts, restored.keys, restored.credentials);
    HostStore::update(cx, |local| restored.apply(local))?;
    cx.global_mut::<SyncService>().hosts = HostStore::get(cx).hosts.clone();
    let prefix = if automatic {
        "Automatic sync complete. "
    } else {
        ""
    };
    Ok(format!(
        "{prefix}Merged {hosts} hosts, {keys} private keys and {credentials} credentials. Deletions are not propagated."
    ))
}

fn finish(result: anyhow::Result<String>, automatic: bool, cx: &mut App) {
    let service = cx.global_mut::<SyncService>();
    service.busy = false;
    service.needs_import = result
        .as_ref()
        .err()
        .is_some_and(|error| error.is::<super::credentials::MissingPasswords>());
    if automatic {
        service
            .queue
            .finished(result.is_ok(), !service.needs_import, Instant::now());
    }
    service.status = Some(match result {
        Ok(message) => {
            log::info!("SSH sync: {message}");
            (Severity::Success, message)
        }
        Err(error) => {
            log::error!("SSH sync: operation failed: {error:#}");
            (Severity::Danger, format!("{error:#}"))
        }
    });
}
