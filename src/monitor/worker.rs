use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::Context as _;

use super::{local::Local, model::Snapshot};
use crate::ssh::Host;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Local,
    Remote {
        host: Host,
        socket: Option<PathBuf>,
        disconnected: bool,
    },
}

pub enum Update {
    Waiting,
    Read(Snapshot),
    Failed(String),
}

pub struct Worker {
    pub receiver: Receiver<Update>,
    cancel: Arc<AtomicBool>,
}

fn rest(cancel: &AtomicBool, duration: Duration) {
    let start = Instant::now();
    while start.elapsed() < duration && !cancel.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(50));
    }
}

impl Worker {
    pub fn start(target: Target) -> anyhow::Result<Self> {
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("system-monitor".into())
            .spawn(move || {
                let mut local = matches!(target, Target::Local).then(Local::new);
                rest(&stop, sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
                let mut waiting = false;
                while !stop.load(Ordering::Relaxed) {
                    let result = match &target {
                        Target::Local => local.as_mut().expect("monitor: local collector").sample(),
                        Target::Remote { host, socket, .. } => {
                            if socket.as_ref().is_some_and(|socket| !socket.exists()) {
                                if !waiting {
                                    let _ = sender.try_send(Update::Waiting);
                                    waiting = true;
                                }
                                rest(&stop, Duration::from_millis(250));
                                continue;
                            }
                            super::remote::sample(host, socket.as_deref(), &stop)
                        }
                    };
                    match result {
                        Ok(snapshot) => {
                            let _ = sender.try_send(Update::Read(snapshot));
                        }
                        Err(error) => {
                            if !stop.load(Ordering::Relaxed) {
                                log::error!("monitor: collection failed: {error:#}");
                                let _ = sender.send(Update::Failed(format!("{error:#}")));
                            }
                            break;
                        }
                    }
                    rest(&stop, Duration::from_secs(2));
                }
                log::debug!("monitor: collector stopped");
            })
            .context("monitor: cannot start the system collector")?;
        Ok(Self { receiver, cancel })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
