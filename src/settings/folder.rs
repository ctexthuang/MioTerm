use std::{ffi::OsString, path::PathBuf};

use anyhow::{Context as _, ensure};

pub(crate) fn folder() -> anyhow::Result<PathBuf> {
    resolve(std::env::var_os("MIOTERM_CONFIG_DIR"))
}

fn resolve(override_path: Option<OsString>) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        let path = PathBuf::from(path);
        ensure!(
            path.is_absolute(),
            "settings: MIOTERM_CONFIG_DIR must be an absolute folder"
        );
        return Ok(path);
    }
    Ok(dirs::config_dir()
        .context("settings: the platform names no config folder")?
        .join("ELYTerminal"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_data_folders_are_absolute_and_do_not_gain_an_extra_suffix() {
        let path = std::env::temp_dir().join("mioterm-portable-fixture");
        assert_eq!(resolve(Some(path.clone().into_os_string())).unwrap(), path);
        for path in ["", "relative-folder", "../other"] {
            assert!(resolve(Some(path.into())).is_err());
        }
    }
}
