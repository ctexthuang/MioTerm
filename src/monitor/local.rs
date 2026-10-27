use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

use super::model::{Disk, Network, Process, Snapshot, Usage};

pub struct Local {
    system: System,
    networks: Networks,
    disks: Disks,
}

impl Local {
    pub fn new() -> Self {
        Self {
            system: System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(sysinfo::CpuRefreshKind::nothing().with_cpu_usage())
                    .with_processes(ProcessRefreshKind::nothing().with_cpu().with_memory()),
            ),
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
        }
    }

    pub fn sample(&mut self) -> anyhow::Result<Snapshot> {
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        self.networks.refresh(true);
        self.disks.refresh(true);
        let mut processes: Vec<_> = self
            .system
            .processes()
            .values()
            .map(|p| Process {
                pid: p.pid().as_u32(),
                command: p.name().to_string_lossy().into_owned(),
                memory: p.memory(),
                cpu: p.cpu_usage().max(0.0),
            })
            .collect();
        processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory)));
        processes.truncate(256);
        let networks = self
            .networks
            .iter()
            .filter(|(name, _)| !matches!(name.as_str(), "lo" | "lo0"))
            .take(256)
            .map(|(name, n)| Network {
                name: name.clone(),
                received: n.total_received(),
                transmitted: n.total_transmitted(),
            })
            .collect();
        let mut disks: Vec<_> = self
            .disks
            .iter()
            .take(256)
            .map(|disk| Disk {
                path: disk.mount_point().to_string_lossy().into_owned(),
                usage: Usage {
                    used: disk.total_space().saturating_sub(disk.available_space()),
                    total: disk.total_space(),
                },
            })
            .collect();
        disks.sort_by(|a, b| a.path.cmp(&b.path));
        #[cfg(unix)]
        let load = {
            let load = System::load_average();
            Some([load.one, load.five, load.fifteen])
        };
        #[cfg(not(unix))]
        let load = None;
        let sample = Snapshot {
            hostname: System::host_name().unwrap_or_else(|| "localhost".into()),
            os: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.into()),
            uptime: System::uptime(),
            load,
            cpu: self.system.global_cpu_usage().clamp(0.0, 100.0),
            memory: Usage {
                used: self.system.used_memory(),
                total: self.system.total_memory(),
            },
            swap: Usage {
                used: self.system.used_swap(),
                total: self.system.total_swap(),
            },
            processes,
            networks,
            disks,
        };
        sample.validate()?;
        Ok(sample)
    }
}
