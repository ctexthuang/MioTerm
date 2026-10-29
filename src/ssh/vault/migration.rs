use std::collections::BTreeMap;

use anyhow::{Context as _, ensure};
use zeroize::Zeroizing;

use super::Vault;
use crate::ssh::{Host, auth};

trait Source {
    fn get(&self, service: &str, account: &str) -> anyhow::Result<Option<Zeroizing<String>>>;
}

struct Native;

impl Source for Native {
    fn get(&self, service: &str, account: &str) -> anyhow::Result<Option<Zeroizing<String>>> {
        let entry = keyring::Entry::new(service, account)
            .context("Credential import: cannot open the system store")?;
        match entry.get_password() {
            Ok(value) => Ok(Some(Zeroizing::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error).context("Credential import: cannot read the system store"),
        }
    }
}

pub(in crate::ssh) struct Report {
    pub imported: usize,
    pub missing: Vec<String>,
}

impl Report {
    pub fn message(&self) -> String {
        let mut text = format!(
            "Imported {} credentials. Original system entries were retained.",
            self.imported
        );
        if !self.missing.is_empty() {
            text.push_str(&format!(
                " No saved credential was found for {}. Re-enter these credentials in the host editor or sync settings; use Restore if they are already in the cloud archive.",
                self.missing.join(", ")
            ));
        }
        text
    }
}

pub(in crate::ssh) fn run(vault: &Vault, hosts: &[Host], token: bool) -> anyhow::Result<Report> {
    copy(vault, hosts, token, &Native)
}

fn copy(
    vault: &Vault,
    hosts: &[Host],
    token: bool,
    source: &impl Source,
) -> anyhow::Result<Report> {
    let mut requested = BTreeMap::new();
    let mut entries = BTreeMap::new();
    for host in hosts
        .iter()
        .filter(|host| host.remember_secret && host.auth.has_secret())
    {
        let account = auth::account(host);
        let key = format!("ssh:{account}");
        if requested.contains_key(&key) {
            continue;
        }
        requested.insert(key.clone(), format!("{:?}", host.label()));
        if let Some(value) = source.get("com.zacharyzhang.elyterminal.ssh", &account)? {
            ensure!(
                !value.is_empty() && value.len() <= 1023 && !value.contains(['\0', '\r', '\n']),
                "Credential import: invalid SSH credential in system store"
            );
            entries.insert(key, value);
        }
    }
    if token {
        requested.insert("sync:gitee".into(), "Gitee access token".into());
        if let Some(value) = source.get("io.github.ctexthuang.mioterm.sync", "gitee")? {
            ensure!(
                !value.is_empty()
                    && value.len() <= 4096
                    && value.bytes().all(|byte| byte.is_ascii_graphic()),
                "Credential import: invalid Gitee token in system store"
            );
            entries.insert("sync:gitee".into(), value);
        }
    }
    let imported = entries.len();
    if imported > 0 {
        vault.insert_many(&entries.into_iter().collect::<Vec<_>>())?;
    }
    let mut missing = Vec::new();
    for (key, label) in requested {
        if vault.get(&key)?.is_none() {
            missing.push(label);
        }
    }
    log::info!(
        "Credential vault: imported {imported} system credentials; {} unresolved; source entries retained",
        missing.len()
    );
    Ok(Report { imported, missing })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::{Authentication, vault::tests::Folder};

    struct Fixture;
    impl Source for Fixture {
        fn get(&self, _: &str, account: &str) -> anyhow::Result<Option<Zeroizing<String>>> {
            Ok(Some(Zeroizing::new(
                if account == "gitee" {
                    "fixture-token"
                } else {
                    "fixture-password"
                }
                .into(),
            )))
        }
    }

    #[test]
    fn explicit_import_is_atomic_and_never_overwrites_a_different_local_secret() {
        let folder = Folder::new();
        let vault = Vault {
            folder: folder.0.join("vault"),
        };
        let host = Host {
            auth: Authentication::Password,
            remember_secret: true,
            ..Host::default()
        };
        let account = format!("ssh:{}", auth::account(&host));
        assert_eq!(
            copy(&vault, &[host.clone(), host.clone()], true, &Fixture)
                .unwrap()
                .imported,
            2
        );
        assert_eq!(
            vault.get(&account).unwrap().unwrap().as_str(),
            "fixture-password"
        );
        vault.put(&account, "different-local-password").unwrap();
        vault.remove("sync:gitee").unwrap();
        assert!(copy(&vault, &[host], true, &Fixture).is_err());
        assert!(vault.get("sync:gitee").unwrap().is_none());
        assert_eq!(
            vault.get(&account).unwrap().unwrap().as_str(),
            "different-local-password"
        );
    }

    struct MissingSsh;
    impl Source for MissingSsh {
        fn get(&self, _: &str, account: &str) -> anyhow::Result<Option<Zeroizing<String>>> {
            Ok((account == "gitee").then(|| Zeroizing::new("fixture-token".into())))
        }
    }

    #[test]
    fn importing_only_a_token_reports_the_unresolved_host_instead_of_success() {
        let folder = Folder::new();
        let vault = Vault::at(folder.0.join("vault"));
        let host = Host {
            name: "Fixture Debian".into(),
            auth: Authentication::Password,
            remember_secret: true,
            ..Host::default()
        };
        let report = copy(&vault, &[host.clone(), host.clone()], true, &MissingSsh).unwrap();
        assert_eq!(report.imported, 1);
        assert_eq!(report.missing, ["\"Fixture Debian\""]);
        assert!(report.message().contains("No saved credential was found"));
        assert!(report.message().contains("Fixture Debian"));
        assert!(vault.get("sync:gitee").unwrap().is_some());
        vault
            .put(
                &format!("ssh:{}", auth::account(&host)),
                "re-entered-fixture",
            )
            .unwrap();
        let report = copy(&vault, &[host], true, &MissingSsh).unwrap();
        assert!(report.missing.is_empty());
        assert!(!report.message().contains("No saved credential"));
    }

    #[test]
    fn a_missing_token_is_reported_and_a_system_read_error_writes_nothing() {
        struct Empty;
        impl Source for Empty {
            fn get(&self, _: &str, _: &str) -> anyhow::Result<Option<Zeroizing<String>>> {
                Ok(None)
            }
        }
        struct Denied;
        impl Source for Denied {
            fn get(&self, _: &str, account: &str) -> anyhow::Result<Option<Zeroizing<String>>> {
                anyhow::ensure!(account != "gitee", "fixture: system access denied");
                Ok(Some(Zeroizing::new("fixture-password".into())))
            }
        }
        let folder = Folder::new();
        let vault = Vault::at(folder.0.join("vault"));
        let report = copy(&vault, &[], true, &Empty).unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.missing, ["Gitee access token"]);
        let host = Host {
            auth: Authentication::Password,
            remember_secret: true,
            ..Host::default()
        };
        assert!(copy(&vault, std::slice::from_ref(&host), true, &Denied).is_err());
        assert!(
            vault
                .get(&format!("ssh:{}", auth::account(&host)))
                .unwrap()
                .is_none()
        );
    }
}
