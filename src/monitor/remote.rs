use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail, ensure};

use super::model::Snapshot;
use crate::ssh::Host;

const SCRIPT: &[u8] = include_bytes!("../../assets/monitor/linux.py");
const LIMIT: u64 = 256 * 1024;

pub fn command(host: &Host, socket: Option<&Path>) -> anyhow::Result<Command> {
    let launch = host.launch(std::env::temp_dir())?;
    let (program, mut args) = launch
        .program
        .context("monitor: SSH launch has no program")?;
    args[0] = "-T".into();
    let mut options = vec![
        "-o".into(),
        if host.remember_secret {
            "BatchMode=no"
        } else {
            "BatchMode=yes"
        }
        .into(),
        "-o".into(),
        "StrictHostKeyChecking=yes".into(),
        "-o".into(),
        "ConnectTimeout=5".into(),
        "-o".into(),
        "ConnectionAttempts=1".into(),
        "-o".into(),
        "ServerAliveInterval=5".into(),
        "-o".into(),
        "ServerAliveCountMax=1".into(),
        "-o".into(),
        "ClearAllForwardings=yes".into(),
        "-o".into(),
        "RemoteCommand=none".into(),
        "-o".into(),
        "PermitLocalCommand=no".into(),
    ];
    if let Some(socket) = socket {
        options.extend([
            "-o".into(),
            "ControlMaster=no".into(),
            "-S".into(),
            socket.display().to_string(),
        ]);
    }
    args.splice(1..1, options);
    args.push("python3 -".into());
    let mut command = Command::new(program);
    command.args(args);
    crate::ssh::auth::configure_command(host, &mut command)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn bounded(mut input: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.by_ref().take(LIMIT + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn collect(command: &mut Command, cancel: &AtomicBool) -> anyhow::Result<Snapshot> {
    let mut child = Running(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("monitor: cannot start SSH collector")?,
    );
    let stdout = child
        .0
        .stdout
        .take()
        .context("monitor: no SSH output pipe")?;
    let stderr = child
        .0
        .stderr
        .take()
        .context("monitor: no SSH error pipe")?;
    let out = thread::spawn(move || bounded(stdout));
    let err = thread::spawn(move || bounded(stderr));
    child
        .0
        .stdin
        .take()
        .context("monitor: no SSH input pipe")?
        .write_all(SCRIPT)
        .context("monitor: cannot send the Linux collector")?;
    let start = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("monitor: collection cancelled");
        }
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "monitor: SSH collector timed out"
        );
        if let Some(status) = child
            .0
            .try_wait()
            .context("monitor: cannot wait for SSH collector")?
        {
            break status;
        }
        thread::sleep(Duration::from_millis(50));
    };
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("monitor: output reader stopped"))?
        .context("monitor: cannot read SSH output")?;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("monitor: error reader stopped"))?
        .context("monitor: cannot read SSH errors")?;
    ensure!(
        stdout.len() as u64 <= LIMIT && stderr.len() as u64 <= LIMIT,
        "monitor: SSH response is too large"
    );
    ensure!(
        status.success(),
        "monitor: remote collection failed: {}",
        String::from_utf8_lossy(&stderr)
            .trim()
            .chars()
            .take(800)
            .collect::<String>()
    );
    let snapshot: Snapshot = serde_json::from_slice(&stdout)
        .context("monitor: server returned invalid system information")?;
    snapshot.validate()?;
    Ok(snapshot)
}

pub fn sample(host: &Host, socket: Option<&Path>, cancel: &AtomicBool) -> anyhow::Result<Snapshot> {
    collect(&mut command(host, socket)?, cancel)
}
