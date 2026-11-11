use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use anyhow::{Context as _, ensure};

use crate::ssh::{files::Transfer, remote::Remote};

pub(super) const LIMIT: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    Queued,
    Running,
    Cancelling,
    Done,
    Failed,
    Cancelled,
}

impl State {
    pub fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Cancelling)
    }
}

pub(super) struct Job {
    pub id: usize,
    pub transfer: Transfer,
    pub state: State,
    pub error: Option<String>,
    pub started: Option<Instant>,
    pub finished: Option<Instant>,
    pub cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct Queue {
    pub jobs: Vec<Job>,
    next: usize,
}

impl Queue {
    pub fn enqueue(&mut self, transfer: Transfer) -> anyhow::Result<()> {
        transfer.batch()?;
        ensure!(
            self.jobs.len() + transfer.items.len() <= LIMIT,
            "sftp: task center holds at most 200 items; clear finished tasks first"
        );
        for item in transfer.items {
            self.next += 1;
            self.jobs.push(Job {
                id: self.next,
                transfer: Transfer {
                    connection: transfer.connection.clone(),
                    upload: transfer.upload,
                    items: vec![item],
                },
                state: State::Queued,
                error: None,
                started: None,
                finished: None,
                cancel: Arc::new(AtomicBool::new(false)),
            });
        }
        Ok(())
    }

    pub fn next(&mut self) -> Option<&Job> {
        if self
            .jobs
            .iter()
            .any(|job| matches!(job.state, State::Running | State::Cancelling))
        {
            return None;
        }
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.state == State::Queued)?;
        job.state = State::Running;
        job.started = Some(Instant::now());
        Some(job)
    }

    pub fn finish(&mut self, id: usize, result: anyhow::Result<()>) -> Option<Remote> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id && matches!(job.state, State::Running | State::Cancelling))?;
        job.finished = Some(Instant::now());
        job.state = match result {
            Ok(()) => State::Done,
            Err(_) if job.cancel.load(Ordering::Relaxed) => State::Cancelled,
            Err(error) => {
                job.error = Some(format!("{error:#}"));
                State::Failed
            }
        };
        log::info!("sftp: task {id} {:?}", job.state);
        if let Some(error) = &job.error {
            log::error!("sftp: task {id}: {error}");
        }
        Some(job.transfer.connection.clone())
    }

    pub fn cancel(&mut self, id: usize) {
        if let Some(job) = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id && job.state.active())
        {
            job.cancel.store(true, Ordering::Relaxed);
            job.state = if job.state == State::Queued {
                State::Cancelled
            } else {
                State::Cancelling
            };
            if job.state == State::Cancelled {
                job.finished = Some(Instant::now());
            }
            log::info!("sftp: task {id} cancellation requested");
        }
    }

    pub fn retry(&mut self, id: usize) -> anyhow::Result<()> {
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .context("sftp: task no longer exists")?;
        ensure!(
            matches!(job.state, State::Failed | State::Cancelled),
            "sftp: only stopped tasks can retry"
        );
        job.state = State::Queued;
        job.error = None;
        job.started = None;
        job.finished = None;
        job.cancel = Arc::new(AtomicBool::new(false));
        log::info!("sftp: task {id} queued for retry");
        Ok(())
    }

    pub fn bind(&mut self, session: u64, remote: Option<&Remote>) {
        for job in self
            .jobs
            .iter_mut()
            .filter(|job| job.transfer.connection.session == session)
        {
            if let Some(remote) = remote {
                let previous = &job.transfer.connection.host;
                if (&previous.address, previous.port, &previous.user)
                    == (&remote.host.address, remote.host.port, &remote.host.user)
                {
                    job.transfer.connection = remote.clone();
                } else {
                    log::warn!(
                        "sftp: task {} retains its original host after a session target change",
                        job.id
                    );
                    job.transfer.connection.socket = None;
                }
            } else {
                job.transfer.connection.socket = None;
            }
        }
    }

    pub fn clear(&mut self) {
        self.jobs.retain(|job| job.state.active());
        log::info!("sftp: finished task history cleared");
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        for job in &self.jobs {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}
