use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
    sync::atomic::AtomicBool,
    time::Duration,
};

use anyhow::Context as _;

use super::{Backend, line};

impl Backend {
    pub(super) fn same_path(&self, a: &Path, b: &Path) -> bool {
        match self {
            Self::Local => a == b,
            Self::Remote(_) => a.as_os_str().as_encoded_bytes() == b.as_os_str().as_encoded_bytes(),
        }
    }

    pub(super) fn contains(&self, folder: &Path, top: &Path) -> bool {
        match self {
            Self::Local => folder.starts_with(top),
            Self::Remote(_) => {
                let (folder, top) = (
                    folder.as_os_str().as_encoded_bytes(),
                    top.as_os_str().as_encoded_bytes(),
                );
                folder == top
                    || folder
                        .strip_prefix(top)
                        .is_some_and(|rest| top.ends_with(b"/") || rest.starts_with(b"/"))
            }
        }
    }

    pub(super) fn git<A: AsRef<OsStr>>(
        &self,
        folder: &Path,
        args: &[A],
        input: &[u8],
    ) -> anyhow::Result<Output> {
        if let Backend::Remote(remote) = self {
            let mut remote_args = vec![
                OsStr::new("env"),
                OsStr::new("LC_ALL=C"),
                OsStr::new("GIT_TERMINAL_PROMPT=0"),
                OsStr::new("git"),
                OsStr::new("-C"),
                folder.as_os_str(),
                OsStr::new("--no-optional-locks"),
                OsStr::new("--literal-pathspecs"),
            ];
            remote_args.extend(args.iter().map(AsRef::as_ref));
            return remote
                .output(
                    &remote_args,
                    input,
                    &AtomicBool::new(false),
                    Duration::from_secs(120),
                )
                .with_context(|| {
                    format!(
                        "git: remote operation on {} failed",
                        remote.host.destination()
                    )
                });
        }
        let named = || format!("git: `git {}` in {}", line(args), folder.display());
        let mut command = command(folder);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            super::CREATE_NO_WINDOW,
        );
        let mut child = command
            .args(["--no-optional-locks", "--literal-pathspecs"])
            .args(args)
            .env("LC_ALL", "C")
            .stdin(if input.is_empty() {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("{} did not start", named()))?;
        let wrote = child
            .stdin
            .take()
            .map_or(Ok(()), |mut stdin| stdin.write_all(input));
        let output = child
            .wait_with_output()
            .with_context(|| format!("{} did not end", named()))?;
        if output.status.success() {
            wrote.with_context(|| format!("{} took no input", named()))?;
        }
        Ok(output)
    }
}

fn command(folder: &Path) -> Command {
    #[cfg(windows)]
    if let Some((share, inside)) = super::wsl::split(folder) {
        let shared =
            std::env::var("WSLENV").map_or("LC_ALL".into(), |list| format!("{list}:LC_ALL"));
        let mut command = Command::new("wsl.exe");
        command
            .args([
                "-d",
                super::wsl::distro(share),
                "--exec",
                "git",
                "-C",
                &inside,
            ])
            .env("WSLENV", shared)
            .env("WSL_UTF8", "1");
        return command;
    }
    let mut command = Command::new("git");
    command.arg("-C").arg(folder);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_paths_keep_posix_identity_on_every_client_platform() {
        let backend = Backend::Remote(crate::ssh::remote::Remote {
            session: 0,
            host: crate::ssh::Host {
                address: "example.test".into(),
                ..Default::default()
            },
            socket: None,
        });
        assert!(!backend.same_path(Path::new("/root/a\\b"), Path::new("/root/a/b")));
        assert!(backend.contains(Path::new("/root/project/src"), Path::new("/root/project")));
        assert!(!backend.contains(Path::new("/root/project-old"), Path::new("/root/project")));
        assert!(backend.contains(Path::new("/root"), Path::new("/")));
    }
}
