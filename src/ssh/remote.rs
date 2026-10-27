use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};

use super::Host;

pub mod process;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub host: Host,
    pub socket: Option<PathBuf>,
    pub session: u64,
}

impl Remote {
    pub fn options(&self) -> Vec<OsString> {
        let mut options: Vec<OsString> = [
            "StrictHostKeyChecking=yes",
            "ConnectTimeout=5",
            "ConnectionAttempts=1",
            "ServerAliveInterval=5",
            "ServerAliveCountMax=2",
            "ClearAllForwardings=yes",
            "RemoteCommand=none",
            "PermitLocalCommand=no",
            "ForwardAgent=no",
            "ForwardX11=no",
        ]
        .into_iter()
        .flat_map(|option| [OsString::from("-o"), option.into()])
        .collect();
        options.extend([
            "-o".into(),
            if self.host.remember_secret {
                "BatchMode=no"
            } else {
                "BatchMode=yes"
            }
            .into(),
        ]);
        if let Some(socket) = &self.socket {
            options.extend(["-o".into(), "ControlMaster=no".into(), "-o".into(), {
                let mut option = OsString::from("ControlPath=");
                option.push(socket);
                option
            }]);
        }
        options
    }

    pub fn wait(&self, cancel: &AtomicBool) -> anyhow::Result<()> {
        let Some(socket) = &self.socket else {
            return Ok(());
        };
        let start = Instant::now();
        while !socket.exists() {
            ensure!(!cancel.load(Ordering::Relaxed), "ssh: operation cancelled");
            ensure!(
                start.elapsed() < Duration::from_secs(45),
                "ssh: finish signing in in the terminal, then refresh this panel"
            );
            thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    pub fn command(&self, args: &[&OsStr]) -> anyhow::Result<Command> {
        let launch = self.host.launch(std::env::temp_dir())?;
        let (program, mut options) = launch.program.context("ssh: launch has no program")?;
        options[0] = "-T".into();
        let mut command = Command::new(program);
        command
            .arg(&options[0])
            .args(self.options())
            .args(&options[1..]);
        command.arg(quoted(args)?);
        super::auth::configure_command(&self.host, &mut command)?;
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            crate::git::CREATE_NO_WINDOW,
        );
        Ok(command)
    }

    pub fn output(
        &self,
        args: &[&OsStr],
        input: &[u8],
        cancel: &AtomicBool,
        timeout: Duration,
    ) -> anyhow::Result<Output> {
        self.wait(cancel)?;
        let output = process::output(&mut self.command(args)?, input, cancel, timeout)?;
        #[cfg(windows)]
        ensure!(
            output.status.code() != Some(255) || self.host.remember_secret,
            "ssh: background Files/Git require an SSH agent or a non-interactive key on Windows; terminal passwords cannot be reused: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(output)
    }
}

fn quoted(args: &[&OsStr]) -> anyhow::Result<OsString> {
    let mut result = Vec::new();
    for arg in args {
        let bytes = arg.as_encoded_bytes();
        ensure!(!bytes.contains(&0), "ssh: command argument contains NUL");
        if !result.is_empty() {
            result.push(b' ');
        }
        result.push(b'\'');
        for &byte in bytes {
            if byte == b'\'' {
                result.extend_from_slice(b"'\\''");
            } else {
                result.push(byte);
            }
        }
        result.push(b'\'');
    }
    #[cfg(unix)]
    return Ok(<OsString as std::os::unix::ffi::OsStringExt>::from_vec(
        result,
    ));
    #[cfg(not(unix))]
    Ok(String::from_utf8(result)
        .context("ssh: argument is not UTF-8")?
        .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_arguments_are_literal_including_quotes_and_newlines() {
        let args = ["git", "a'; touch /tmp/no", "line\nbreak", "$(echo no)", ""];
        let args = args.map(OsStr::new);
        assert_eq!(
            quoted(&args).unwrap(),
            "'git' 'a'\\''; touch /tmp/no' 'line\nbreak' '$(echo no)' ''"
        );
        assert!(quoted(&[OsStr::new("a\0b")]).is_err());
    }

    #[test]
    fn background_connections_never_prompt_or_accept_unknown_hosts() {
        let remote = Remote {
            session: 0,
            host: Host {
                address: "example.test".into(),
                ..Host::default()
            },
            socket: Some("/tmp/private/s".into()),
        };
        let args = remote.command(&[OsStr::new("pwd")]).unwrap();
        let args: Vec<_> = args.get_args().map(|arg| arg.to_string_lossy()).collect();
        assert!(args.iter().any(|arg| arg == "BatchMode=yes"));
        assert!(args.iter().any(|arg| arg == "StrictHostKeyChecking=yes"));
        assert!(args.iter().any(|arg| arg == "ControlPath=/tmp/private/s"));
        assert_eq!(args.last().unwrap(), "'pwd'");
    }
}
