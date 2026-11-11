use std::path::PathBuf;

use super::model::{LIMIT, Queue, State};
use crate::ssh::{
    Host,
    files::{RemotePath, Transfer, TransferItem},
    remote::Remote,
};

fn transfer(count: usize) -> Transfer {
    Transfer {
        connection: Remote {
            host: Host::default(),
            socket: Some(PathBuf::from("/tmp/fixture/s")),
            session: 1,
        },
        upload: true,
        items: (0..count)
            .map(|i| TransferItem {
                local: std::env::temp_dir().join(format!("fixture-{i}")),
                remote: RemotePath(format!("/fixture-{i}").into_bytes()),
                directory: false,
            })
            .collect(),
    }
}

#[test]
fn tasks_are_serial_and_one_failure_does_not_skip_other_items() {
    let mut queue = Queue::default();
    queue.enqueue(transfer(3)).unwrap();
    let id = queue.next().unwrap().id;
    assert!(queue.next().is_none());
    queue
        .finish(id, Err(anyhow::anyhow!("fixture transfer failed")))
        .unwrap();
    assert_eq!(queue.jobs[0].state, State::Failed);
    assert!(
        queue.jobs[0]
            .error
            .as_ref()
            .unwrap()
            .contains("fixture transfer failed")
    );
    let id = queue.next().unwrap().id;
    queue.finish(id, Ok(())).unwrap();
    assert_eq!(queue.jobs[1].state, State::Done);
    assert_eq!(queue.jobs[2].state, State::Queued);
    queue.clear();
    assert_eq!(queue.jobs.len(), 1);
}

#[test]
fn cancelling_queued_and_running_tasks_then_retrying_is_consistent() {
    let mut queue = Queue::default();
    queue.enqueue(transfer(2)).unwrap();
    let id = queue.next().unwrap().id;
    queue.cancel(id);
    assert_eq!(queue.jobs[0].state, State::Cancelling);
    assert!(queue.next().is_none());
    queue.finish(id, Err(anyhow::anyhow!("cancelled"))).unwrap();
    assert_eq!(queue.jobs[0].state, State::Cancelled);
    assert!(queue.jobs[0].error.is_none());
    queue.cancel(queue.jobs[1].id);
    assert_eq!(queue.jobs[1].state, State::Cancelled);
    queue.retry(id).unwrap();
    assert!(
        !queue.jobs[0]
            .cancel
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    let id = queue.next().unwrap().id;
    queue.cancel(id);
    queue.finish(id, Ok(())).unwrap();
    assert_eq!(queue.jobs[0].state, State::Done);
}

#[test]
fn reconnect_rebinds_only_its_own_tasks_and_closed_sessions_drop_stale_sockets() {
    let mut queue = Queue::default();
    queue.enqueue(transfer(1)).unwrap();
    let mut other = transfer(1);
    other.connection.session = 2;
    queue.enqueue(other).unwrap();
    let mut remote = transfer(1).connection;
    remote.session = 3;
    remote.socket = Some("/tmp/new/s".into());
    queue.bind(1, Some(&remote));
    assert_eq!(queue.jobs[0].transfer.connection, remote);
    assert_eq!(queue.jobs[1].transfer.connection.session, 2);
    queue.bind(3, None);
    assert!(queue.jobs[0].transfer.connection.socket.is_none());
    assert!(queue.jobs[1].transfer.connection.socket.is_some());
}

#[test]
fn limits_and_validation_are_atomic_and_completed_records_can_be_cleared() {
    let mut queue = Queue::default();
    let mut invalid = transfer(2);
    invalid.items[1].remote.0.push(b'\n');
    assert!(queue.enqueue(invalid).is_err());
    assert!(queue.jobs.is_empty());
    queue.enqueue(transfer(LIMIT)).unwrap();
    assert!(queue.enqueue(transfer(1)).is_err());
    assert_eq!(queue.jobs.len(), LIMIT);
    for job in &mut queue.jobs {
        job.state = State::Done;
    }
    queue.clear();
    queue.enqueue(transfer(1)).unwrap();
    assert!(queue.jobs[0].id > LIMIT);
}

#[test]
fn reconnect_cannot_redirect_queued_transfers_to_another_server() {
    let mut queue = Queue::default();
    queue.enqueue(transfer(1)).unwrap();
    let original = queue.jobs[0].transfer.connection.host.clone();
    let mut remote = transfer(1).connection;
    remote.host.address = "another-server.invalid".into();
    remote.session = 99;
    queue.bind(1, Some(&remote));
    assert_eq!(queue.jobs[0].transfer.connection.host, original);
    assert_eq!(queue.jobs[0].transfer.connection.session, 1);
    assert!(queue.jobs[0].transfer.connection.socket.is_none());
}

#[test]
fn finishing_after_reconnect_refreshes_the_current_session() {
    let mut queue = Queue::default();
    queue.enqueue(transfer(1)).unwrap();
    let id = queue.next().unwrap().id;
    let mut remote = transfer(1).connection;
    remote.session = 3;
    remote.socket = Some("/tmp/reconnected/s".into());
    queue.bind(1, Some(&remote));
    assert_eq!(queue.finish(id, Ok(())).unwrap(), remote);
    assert!(queue.finish(id, Ok(())).is_none());
}
