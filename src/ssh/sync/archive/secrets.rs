use std::{collections::BTreeMap, path::Path};

use anyhow::{Context as _, ensure};

use super::{Archive, Connections, Credential, PortableHost, Store};

struct Pending<'a> {
    id: u64,
    credential: &'a Credential,
    existing: Option<zeroize::Zeroizing<String>>,
    differs: bool,
}

impl Archive {
    pub fn capture_credentials(
        &mut self,
        local: &Connections,
        store: &impl Store,
    ) -> anyhow::Result<Vec<String>> {
        let mut warnings = Vec::new();
        for host in &local.hosts {
            if !self.hosts.iter().any(|portable| portable.same_fields(host)) {
                continue;
            }
            if host.remember_secret && host.auth.has_secret() {
                match store.load(host)? {
                    Some(secret) => {
                        self.credentials.insert(
                            PortableHost::capture(host)?.id(),
                            Credential(secret.to_string()),
                        );
                    }
                    None if host.auth == crate::ssh::Authentication::Password => {
                        let warning = format!(
                            "No saved SSH password for {:?}; exporting connection details only.",
                            host.label()
                        );
                        log::warn!("SSH sync: {warning}");
                        warnings.push(warning);
                    }
                    None => {}
                }
            }
        }
        self.validate()?;
        Ok(warnings)
    }

    #[cfg(test)]
    pub fn restore_credentials(
        &self,
        local: &mut Connections,
        store: &impl Store,
    ) -> anyhow::Result<usize> {
        self.restore_secrets(local, store, None)
    }

    pub(in crate::ssh::sync) fn restore_secrets(
        &self,
        local: &mut Connections,
        store: &impl Store,
        before: Option<&Archive>,
    ) -> anyhow::Result<usize> {
        let pending = self.pending(local, store, before)?;
        for item in &pending {
            let host = local
                .hosts
                .iter_mut()
                .find(|host| host.id == item.id)
                .expect("restored SSH host");
            let current = store.load(host)?;
            if current.as_deref().map(String::as_str) != Some(item.credential.0.as_str()) {
                ensure!(
                    current == item.existing,
                    "SSH sync: local credential changed during synchronization; retry"
                );
                if item.differs {
                    store.replace(host, item.existing.as_ref().unwrap(), &item.credential.0)?;
                } else if item.existing.is_none() {
                    store.insert(host, &item.credential.0)?;
                }
            }
            host.remember_secret = true;
        }
        Ok(pending.len())
    }

    pub(in crate::ssh::sync) fn check_secrets(
        &self,
        local: &Connections,
        store: &impl Store,
        before: &Archive,
    ) -> anyhow::Result<usize> {
        Ok(self.pending(local, store, Some(before))?.len())
    }

    fn pending<'a>(
        &'a self,
        local: &Connections,
        store: &impl Store,
        before: Option<&Archive>,
    ) -> anyhow::Result<Vec<Pending<'a>>> {
        let mut pending = Vec::new();
        let mut accounts = BTreeMap::new();
        for host in &local.hosts {
            if !host.identity.is_empty()
                && !Path::new(&host.identity).try_exists().with_context(|| {
                    format!("SSH sync: cannot inspect the key for {}", host.label())
                })?
            {
                continue;
            }
            let portable = PortableHost::capture(host)?;
            if !self.hosts.contains(&portable) {
                continue;
            }
            if local.is_deleted(host, portable.key.as_deref()) {
                continue;
            }
            if let Some(credential) = self.credentials.get(&portable.id()) {
                let previous = accounts.insert(crate::ssh::auth::account(host), credential);
                ensure!(
                    previous.is_none_or(|old| old == credential),
                    "SSH sync: profiles sharing one credential account have conflicting credentials"
                );
                let existing = store.load(host)?;
                let differs = existing
                    .as_ref()
                    .is_some_and(|secret| secret.as_str() != credential.0);
                if differs {
                    let previous = before.and_then(|archive| {
                        archive.hosts.iter().find(|old| old.id() == portable.id())
                    });
                    ensure!(
                        previous.is_some_and(|old| old.sync.version() < portable.sync.version())
                            && before
                                .and_then(|archive| archive.credentials.get(&portable.id()))
                                .is_some_and(|old| Some(old.0.as_str())
                                    == existing.as_deref().map(String::as_str)),
                        "SSH sync: a local credential changed or differs at the same revision; it was not overwritten"
                    );
                }
                pending.push(Pending {
                    id: host.id,
                    credential,
                    existing,
                    differs,
                });
            }
        }
        Ok(pending)
    }
}
