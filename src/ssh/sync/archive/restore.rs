use std::{collections::BTreeMap, path::Path};

use super::{Archive, Connections, Key};

impl Archive {
    pub fn restore(&self, mut local: Connections, folder: &Path) -> anyhow::Result<Connections> {
        self.validate()?;
        local.migrate_records()?;
        local.records.deleted.extend(self.deleted.clone());
        local.deleted_hosts.extend(self.exclusions.clone());
        let previous_count = local.hosts.len();
        local
            .hosts
            .retain(|host| !local.records.deleted.contains(&host.sync.id));
        if previous_count != local.hosts.len() {
            log::info!(
                "SSH sync: applied {} remote deletions",
                previous_count - local.hosts.len()
            );
        }
        let hosts = self.visible_hosts(&local);
        let mut existing = BTreeMap::new();
        for host in &local.hosts {
            if !host.identity.is_empty() && Path::new(&host.identity).is_file() {
                existing.insert(host.id, Key::capture(Path::new(&host.identity))?.id());
            }
        }
        for portable in hosts {
            let mut portable = portable.clone();
            portable.identify();
            local.records.clock = local.records.clock.max(portable.sync.revision);
            let same = local
                .hosts
                .iter()
                .find(|host| {
                    host.sync.id == portable.sync.id
                        || (host.sync.revision == 0
                            && portable.sync.revision == 0
                            && portable.same_fields(host)
                            && match &portable.key {
                                None => host.identity.is_empty(),
                                Some(key) => {
                                    existing.get(&host.id) == Some(key)
                                        || (!host.identity.is_empty()
                                            && !Path::new(&host.identity).exists())
                                }
                            })
                })
                .cloned();
            if same
                .as_ref()
                .is_some_and(|host| host.sync.version() > portable.sync.version())
            {
                continue;
            }
            let identity = match &portable.key {
                Some(key)
                    if same
                        .as_ref()
                        .is_some_and(|host| existing.get(&host.id) == Some(key)) =>
                {
                    same.as_ref().unwrap().identity.clone()
                }
                Some(key) => self.keys[key].install(folder)?,
                None => String::new(),
            };
            let mut host = portable.host(identity);
            if let Some(saved) = same {
                host.id = saved.id;
                host.keepalive = saved.keepalive;
                host.remember_secret = host.auth.has_secret()
                    && saved.remember_secret
                    && crate::ssh::auth::account(&saved) == crate::ssh::auth::account(&host);
                if host != saved {
                    log::info!("SSH sync: updating saved record {}", host.id);
                }
            } else {
                log::info!("SSH sync: importing connection {:?}", host.label());
            }
            local.merge_host(host)?;
        }
        local
            .recent
            .retain(|recent| !local.records.deleted.contains(&recent.host.sync.id));
        local.validate()?;
        Ok(local)
    }
}
