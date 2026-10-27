#[cfg(unix)]
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[cfg(unix)]
use super::remote;
use super::{local::Local, model::*};

#[test]
fn both_sidebars_start_closed_and_ely_stays_enabled() {
    let defaults = crate::settings::Settings::default();
    assert!(!defaults.show_sessions);
    assert!(!defaults.show_panel);
    assert!(defaults.ely);
    assert!(!super::visible(&defaults));
}

#[test]
fn monitor_fills_follow_the_current_theme_and_warning_levels() {
    for theme in crate::themes::all() {
        let colors = crate::themes::palette(theme, 1.0);
        assert_eq!(
            super::meter::color(0.3, &colors),
            colors.accent,
            "{}",
            theme.id
        );
        assert_eq!(super::meter::color(0.8, &colors), colors.warning);
        assert_eq!(super::meter::color(0.95, &colors), colors.danger);
    }
}

#[test]
fn monitoring_only_runs_in_the_visible_monitor_tab() {
    use crate::settings::{PanelTab, Settings};
    for panel in [PanelTab::Git, PanelTab::Files, PanelTab::Monitor] {
        for show_panel in [false, true] {
            for show_sessions in [false, true] {
                let settings = Settings {
                    panel,
                    show_panel,
                    show_sessions,
                    ..Default::default()
                };
                assert_eq!(
                    super::visible(&settings),
                    show_panel && panel == PanelTab::Monitor
                );
            }
        }
    }
}

#[test]
fn right_panel_selection_survives_hiding_and_settings_round_trip() {
    use crate::settings::{PanelTab, Settings};
    for panel in [PanelTab::Git, PanelTab::Files, PanelTab::Monitor] {
        let settings = Settings {
            panel,
            show_panel: false,
            ..Default::default()
        };
        let encoded = serde_json::to_string(&settings).unwrap();
        let restored: Settings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored.panel, panel);
        assert!(!restored.show_panel);
        assert!(restored.ely);
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        hostname: "debian".into(),
        os: "Linux".into(),
        uptime: 80_000,
        load: Some([0.03, 0.09, 0.08]),
        cpu: 1.0,
        memory: Usage {
            used: 1300,
            total: 7800,
        },
        swap: Usage {
            used: 14,
            total: 3100,
        },
        processes: vec![Process {
            pid: 10,
            command: "zsh".into(),
            memory: 300,
            cpu: 0.5,
        }],
        networks: vec![Network {
            name: "eth0".into(),
            received: 100,
            transmitted: 200,
        }],
        disks: vec![Disk {
            path: "/".into(),
            usage: Usage {
                used: 100,
                total: 200,
            },
        }],
    }
}

#[test]
fn remote_samples_are_structured_and_validated() {
    let encoded = serde_json::to_vec(&snapshot()).unwrap();
    let decoded: Snapshot = serde_json::from_slice(&encoded).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded.hostname, "debian");
    assert_eq!(decoded.processes[0].pid, 10);
}

#[test]
fn impossible_remote_metrics_are_rejected() {
    let mut sample = snapshot();
    sample.cpu = f32::NAN;
    assert!(sample.validate().is_err());
    sample.cpu = 101.0;
    assert!(sample.validate().is_err());
    sample.cpu = 1.0;
    sample.memory.used = sample.memory.total + 1;
    assert!(sample.validate().is_err());
    sample.memory.used = 0;
    sample.networks.push(sample.networks[0].clone());
    assert!(sample.validate().is_err());
}

#[test]
fn huge_or_non_finite_records_are_rejected() {
    let mut sample = snapshot();
    sample.load = Some([f64::INFINITY, 0.0, 0.0]);
    assert!(sample.validate().is_err());
    sample.load = None;
    sample.processes[0].command = "x".repeat(4097);
    assert!(sample.validate().is_err());
    sample.processes[0].command.clear();
    sample.processes[0].cpu = -1.0;
    assert!(sample.validate().is_err());
}

#[test]
fn traffic_uses_elapsed_time_and_handles_counter_resets() {
    let now = Instant::now();
    let mut traffic = Traffic::default();
    let mut networks = snapshot().networks;
    assert!(traffic.sample(&networks, now).is_empty());
    networks[0].received += 400;
    networks[0].transmitted += 600;
    let rates = traffic.sample(&networks, now + Duration::from_secs(2));
    assert_eq!(rates["eth0"].received, 200.0);
    assert_eq!(rates["eth0"].transmitted, 300.0);
    networks[0].received = 0;
    let rates = traffic.sample(&networks, now + Duration::from_secs(4));
    assert_eq!(rates["eth0"].received, 0.0);
}

#[test]
fn a_new_interface_has_no_fabricated_transfer_rate() {
    let now = Instant::now();
    let mut traffic = Traffic::default();
    traffic.sample(&[], now);
    assert!(
        traffic
            .sample(&snapshot().networks, now + Duration::from_secs(2))
            .is_empty()
    );
}

#[test]
fn units_and_uptime_are_consistent() {
    assert_eq!(bytes(0.0), "0 B");
    assert_eq!(bytes(1024.0), "1.0 KiB");
    assert_eq!(bytes(1024.0 * 1024.0 * 1024.0), "1.0 GiB");
    assert_eq!(compact_bytes(1024.0 * 870.4), "870K");
    assert_eq!(compact_bytes(1024.0 * 25.7), "25.7K");
    assert_eq!(uptime(23 * 3600 + 60), "23h 1m");
    assert_eq!(uptime(2 * 86400 + 3600), "2d 1h");
}

#[test]
fn local_monitor_reads_real_metrics() {
    let mut collector = Local::new();
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    let sample = collector.sample().unwrap();
    assert!(sample.memory.total > 0);
    assert!(sample.memory.used <= sample.memory.total);
    sample.validate().unwrap();
}

#[cfg(unix)]
#[test]
fn remote_monitor_never_prompts_or_accepts_unknown_host_keys() {
    let host = crate::ssh::Host {
        address: "example.invalid".into(),
        user: "root".into(),
        port: 2222,
        ..Default::default()
    };
    let command = remote::command(&host, Some(std::path::Path::new("/tmp/a socket"))).unwrap();
    let args: Vec<_> = command.get_args().map(|s| s.to_str().unwrap()).collect();
    assert_eq!(args[0], "-T");
    assert!(args.contains(&"BatchMode=yes"));
    assert!(args.contains(&"StrictHostKeyChecking=yes"));
    assert!(args.contains(&"/tmp/a socket"));
    assert_eq!(
        &args[args.len() - 3..],
        ["--", "example.invalid", "python3 -"]
    );
}

#[cfg(unix)]
fn fixture(output: &str, status: u8) -> std::process::Command {
    let mut command = std::process::Command::new("/bin/sh");
    command.args([
        "-c",
        "cat >/dev/null; printf '%s' \"$1\"; exit \"$2\"",
        "monitor-test",
        output,
        &status.to_string(),
    ]);
    command
}

#[cfg(unix)]
#[test]
fn collector_reads_bounded_json_and_rejects_invalid_output() {
    let cancel = AtomicBool::new(false);
    let json = serde_json::to_string(&snapshot()).unwrap();
    assert_eq!(
        remote::collect(&mut fixture(&json, 0), &cancel)
            .unwrap()
            .hostname,
        "debian"
    );
    assert!(remote::collect(&mut fixture("not json", 0), &cancel).is_err());
    assert!(remote::collect(&mut fixture(&json, 1), &cancel).is_err());
    let mut oversized = std::process::Command::new("/bin/sh");
    oversized.args(["-c", "cat >/dev/null; head -c 300000 /dev/zero"]);
    let error = remote::collect(&mut oversized, &cancel).err().unwrap();
    assert!(error.to_string().contains("too large"), "{error:#}");
}

#[cfg(unix)]
#[test]
fn cancelling_a_slow_collector_kills_its_child() {
    use std::sync::{Arc, atomic::Ordering};
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        flag.store(true, Ordering::Relaxed);
    });
    let mut command = std::process::Command::new("/bin/sh");
    command.args(["-c", "cat >/dev/null; exec sleep 30"]);
    let start = Instant::now();
    assert!(remote::collect(&mut command, &cancel).is_err());
    setter.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
}
