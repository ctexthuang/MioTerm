use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Context as _;
use gpui::{App, Global};

use super::Connections;

pub struct HostStore {
    data: Connections,
    path: PathBuf,
}

impl Global for HostStore {}

impl HostStore {
    pub fn init(cx: &mut App) -> anyhow::Result<()> {
        let path = dirs::config_dir()
            .context("ssh: the platform names no config folder")?
            .join("ELYTerminal/connections.json");
        let data = load(&path)?;
        cx.set_global(Self { data, path });
        Ok(())
    }

    pub fn get(cx: &App) -> &Connections {
        &cx.global::<Self>().data
    }

    pub fn update<T>(
        cx: &mut App,
        edit: impl FnOnce(&mut Connections) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let store = cx.global::<Self>();
        let mut next = store.data.clone();
        let result = edit(&mut next)?;
        next.validate()?;
        save(&store.path, &next)?;
        cx.global_mut::<Self>().data = next;
        Ok(result)
    }
}

pub(super) fn load(path: &Path) -> anyhow::Result<Connections> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            log::info!("ssh: no connections at {}, starting empty", path.display());
            return Ok(Connections::default());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("ssh: cannot read {}", path.display()));
        }
    };
    let data: Connections = serde_json::from_str(&text)
        .with_context(|| format!("ssh: invalid connections at {}", path.display()))?;
    data.validate()
        .with_context(|| format!("ssh: invalid connections at {}", path.display()))?;
    log::info!(
        "ssh: loaded {} hosts and {} recent connections from {}",
        data.hosts.len(),
        data.recent.len(),
        path.display()
    );
    Ok(data)
}

pub(super) fn save(path: &Path, data: &Connections) -> anyhow::Result<()> {
    static SAVES: AtomicU64 = AtomicU64::new(0);
    data.validate()?;
    let folder = path.parent().context("ssh: connections have no folder")?;
    fs::create_dir_all(folder)
        .with_context(|| format!("ssh: cannot create {}", folder.display()))?;
    let temp = path.with_extension(format!(
        "json.{}-{}.tmp",
        std::process::id(),
        SAVES.fetch_add(1, Ordering::Relaxed)
    ));
    let mut text = serde_json::to_string_pretty(data).context("ssh: cannot encode connections")?;
    text.push('\n');
    let write = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temp, path));
    if let Err(error) = write {
        if temp.exists()
            && let Err(error) = fs::remove_file(&temp)
        {
            log::error!(
                "ssh: temporary connections stay at {}: {error}",
                temp.display()
            );
        }
        return Err(error).with_context(|| format!("ssh: cannot save {}", path.display()));
    }
    log::info!("ssh: saved connections to {}", path.display());
    Ok(())
}
