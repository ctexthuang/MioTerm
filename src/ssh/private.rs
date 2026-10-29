use std::{fs, path::Path};

use anyhow::{Context as _, ensure};

#[cfg(windows)]
mod windows;

pub(super) fn regular(path: &Path, folder: bool) -> anyhow::Result<()> {
    let metadata =
        fs::symlink_metadata(path).context("SSH storage: cannot inspect private path")?;
    ensure!(
        !metadata.file_type().is_symlink()
            && if folder {
                metadata.is_dir()
            } else {
                metadata.is_file()
            },
        "SSH storage: private path is a symlink or has the wrong type"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "SSH storage: private path is a reparse point"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() } && (folder || metadata.nlink() == 1),
            "SSH storage: private path has another owner or multiple hard links"
        );
    }
    Ok(())
}

pub(super) fn folder(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(path)
                    .context("SSH storage: cannot create private folder")?;
            }
            #[cfg(not(unix))]
            fs::create_dir_all(path).context("SSH storage: cannot create private folder")?;
        }
        Err(error) => return Err(error).context("SSH storage: cannot inspect private folder"),
    }
    regular(path, true)?;
    protect(path, true)
}

#[cfg(unix)]
pub(super) fn protect(path: &Path, folder: bool) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if folder { 0o700 } else { 0o600 }),
    )
    .context("SSH storage: cannot restrict private path permissions")
}

#[cfg(windows)]
pub(super) fn protect(path: &Path, folder: bool) -> anyhow::Result<()> {
    windows::protect(path, folder)
}
