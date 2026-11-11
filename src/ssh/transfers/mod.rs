use std::time::Duration;

use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, Task};

use super::{files::Transfer, remote::Remote};
use model::Queue;

mod model;
#[cfg(test)]
mod tests;
mod view;

struct Store(Entity<Transfers>);
impl Global for Store {}

pub struct Finished(pub Remote);

pub struct Transfers {
    queue: Queue,
    worker: Option<Task<()>>,
    _clock: Task<()>,
}

impl EventEmitter<Finished> for Transfers {}

impl Transfers {
    pub fn get(cx: &mut App) -> Entity<Self> {
        if let Some(store) = cx.try_global::<Store>() {
            return store.0.clone();
        }
        let entity = cx.new(|cx: &mut Context<Self>| Self {
            queue: Queue::default(),
            worker: None,
            _clock: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    if this
                        .update(cx, |this, cx| {
                            if this.worker.is_some() {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }),
        });
        cx.set_global(Store(entity.clone()));
        entity
    }

    pub fn enqueue(&mut self, transfer: Transfer, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let count = transfer.items.len();
        self.queue.enqueue(transfer)?;
        log::info!("sftp: queued {count} transfer tasks");
        self.start(cx);
        cx.notify();
        Ok(())
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.worker.is_some() {
            return;
        }
        let Some(job) = self.queue.next() else {
            return;
        };
        let (id, transfer, cancel) = (job.id, job.transfer.clone(), job.cancel.clone());
        log::info!(
            "sftp: task {id} started on {}",
            transfer.connection.host.destination()
        );
        let task = cx.background_spawn(async move { transfer.run(&cancel) });
        self.worker = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let connection = this.queue.finish(id, result);
                this.worker = None;
                if let Some(connection) = connection {
                    cx.emit(Finished(connection));
                }
                this.start(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    pub fn bind(&mut self, session: u64, remote: Option<&Remote>, cx: &mut Context<Self>) {
        self.queue.bind(session, remote);
        cx.notify();
    }

    pub fn busy(&self, connection: &Remote) -> bool {
        self.queue
            .jobs
            .iter()
            .any(|job| job.state.active() && job.transfer.connection.session == connection.session)
    }

    pub fn active(&self) -> usize {
        self.queue
            .jobs
            .iter()
            .filter(|job| job.state.active())
            .count()
    }

    fn retry(&mut self, id: usize, cx: &mut Context<Self>) {
        if let Err(error) = self.queue.retry(id) {
            log::error!("{error:#}");
        }
        self.start(cx);
        cx.notify();
    }
}
