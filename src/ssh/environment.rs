use std::{collections::HashSet, ffi::OsStr, process::Command};

fn is_locale(name: &OsStr) -> bool {
    let name = name.as_encoded_bytes();
    #[cfg(windows)]
    let folded = name.to_ascii_uppercase();
    #[cfg(windows)]
    let name = folded.as_slice();
    name == b"LANG" || name == b"LANGUAGE" || name.starts_with(b"LC_")
}

pub(crate) fn configure(command: &mut Command) {
    let names: HashSet<_> = std::env::vars_os()
        .map(|(name, _)| name)
        .chain(command.get_envs().map(|(name, _)| name.to_owned()))
        .filter(|name| is_locale(name))
        .collect();
    for name in names {
        command.env_remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_locale_variables_are_removed_from_the_child_environment() {
        let mut command = Command::new("ssh");
        for name in ["LANG", "LANGUAGE", "LC_ALL", "LC_TIME", "LC_CUSTOM"] {
            command.env(name, "mioterm_missing_locale");
        }
        command.env("TERM", "xterm-256color");
        command.env("SSH_AUTH_SOCK", "fixture-agent");
        configure(&mut command);
        for name in ["LANG", "LANGUAGE", "LC_ALL", "LC_TIME", "LC_CUSTOM"] {
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == name && value.is_none())
            );
        }
        for name in ["TERM", "SSH_AUTH_SOCK"] {
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == name && value.is_some())
            );
        }
    }

    #[test]
    fn locale_names_follow_the_platforms_case_rules() {
        for name in ["LANG", "LANGUAGE", "LC_CTYPE"] {
            assert!(is_locale(OsStr::new(name)));
        }
        for name in ["PATH", "TERM", "LC", "LANG_OTHER"] {
            assert!(!is_locale(OsStr::new(name)));
        }
        assert_eq!(is_locale(OsStr::new("lc_all")), cfg!(windows));
    }
}
