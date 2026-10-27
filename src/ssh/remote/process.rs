use std::{
    io::{Read, Write},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};

const LIMIT: usize = 16 * 1024 * 1024;

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn read(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.by_ref()
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn output(
    command: &mut Command,
    input: &[u8],
    cancel: &AtomicBool,
    timeout: Duration,
) -> anyhow::Result<Output> {
    let mut child = Running(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("ssh: cannot start background operation")?,
    );
    let stdout = child.0.stdout.take().context("ssh: no output pipe")?;
    let stderr = child.0.stderr.take().context("ssh: no error pipe")?;
    let mut stdin = child.0.stdin.take().context("ssh: no input pipe")?;
    let input = input.to_vec();
    let out = thread::spawn(move || read(stdout));
    let err = thread::spawn(move || read(stderr));
    let wrote = thread::spawn(move || stdin.write_all(&input));
    let start = Instant::now();
    let status = loop {
        ensure!(!cancel.load(Ordering::Relaxed), "ssh: operation cancelled");
        ensure!(start.elapsed() < timeout, "ssh: operation timed out");
        if let Some(status) = child
            .0
            .try_wait()
            .context("ssh: cannot wait for operation")?
        {
            break status;
        }
        thread::sleep(Duration::from_millis(30));
    };
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("ssh: output reader stopped"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("ssh: error reader stopped"))??;
    let wrote = wrote
        .join()
        .map_err(|_| anyhow::anyhow!("ssh: input writer stopped"))?;
    ensure!(
        stdout.len() <= LIMIT && stderr.len() <= LIMIT,
        "ssh: response exceeds 16 MiB"
    );
    if status.success() {
        wrote.context("ssh: operation did not take its input")?;
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn pipes_are_drained_while_input_is_written() {
        let mut command = Command::new("sh");
        command.args(["-c", "head -c 1048576 /dev/zero; cat"]);
        let input = vec![b'x'; 1024 * 1024];
        let out = output(
            &mut command,
            &input,
            &AtomicBool::new(false),
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 2 * 1024 * 1024);
        assert_eq!(&out.stdout[1024 * 1024..], &input);
    }

    #[test]
    fn hung_and_cancelled_operations_are_stopped() {
        let mut command = Command::new("sleep");
        command.arg("30");
        assert!(
            output(
                &mut command,
                &[],
                &AtomicBool::new(false),
                Duration::from_millis(50)
            )
            .is_err()
        );
        assert!(
            output(
                &mut command,
                &[],
                &AtomicBool::new(true),
                Duration::from_secs(5)
            )
            .is_err()
        );
    }
}
