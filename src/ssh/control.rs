use std::path::Path;

use crate::term::Launch;

/// A session's private OpenSSH socket, never a password or a key.
pub struct Control {
    #[cfg(unix)]
    folder: std::path::PathBuf,
}

impl Control {
    pub fn new() -> anyhow::Result<Self> {
        #[cfg(unix)]
        {
            use anyhow::Context as _;
            use std::{fs::DirBuilder, os::unix::fs::DirBuilderExt, time::SystemTime};
            let stamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_nanos();
            let folder = std::path::PathBuf::from("/tmp")
                .join(format!("ely-ssh-{}-{stamp:x}", std::process::id()));
            DirBuilder::new()
                .mode(0o700)
                .create(&folder)
                .context("ssh: cannot create the private connection directory")?;
            log::debug!(
                "ssh: private control directory opens at {}",
                folder.display()
            );
            Ok(Self { folder })
        }
        #[cfg(not(unix))]
        Ok(Self {})
    }

    pub fn socket(&self) -> Option<std::path::PathBuf> {
        #[cfg(unix)]
        return Some(self.folder.join("s"));
        #[cfg(not(unix))]
        None
    }

    pub fn configure(&self, launch: &mut Launch) {
        if let Some(socket) = self.socket()
            && let Some((_, args)) = &mut launch.program
        {
            args.splice(
                1..1,
                [
                    "-o".into(),
                    "ControlMaster=yes".into(),
                    "-o".into(),
                    "ControlPersist=no".into(),
                    "-S".into(),
                    socket.display().to_string(),
                ],
            );
        }
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        if let Some(socket) = self.socket() {
            remove(&socket, false);
            if let Some(folder) = socket.parent() {
                remove(folder, true);
            }
        }
    }
}

fn remove(path: &Path, directory: bool) {
    let result = if directory {
        std::fs::remove_dir(path)
    } else {
        std::fs::remove_file(path)
    };
    if let Err(error) = result
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!(
            "ssh: cannot remove control path {}: {error}",
            path.display()
        );
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn sockets_are_private_unique_and_removed_with_their_session() {
        let first = Control::new().unwrap();
        let second = Control::new().unwrap();
        let socket = first.socket().unwrap();
        assert_ne!(Some(&socket), second.socket().as_ref());
        let folder = socket.parent().unwrap();
        assert_eq!(
            std::fs::metadata(folder).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(socket.as_os_str().len() < 100);
        drop(first);
        assert!(!folder.exists());
    }
}
