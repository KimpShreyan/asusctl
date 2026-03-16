use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use dmi_id::DMIID;
use slint::{ComponentHandle, ModelRc, VecModel, Weak};

use crate::{DashboardPageData, FanModeEntry, MainWindow, MonitorEntry, ProductInfo};

/// Set static product info from DMI and procfs (called once at startup).
pub fn setup_dashboard_page(ui: &MainWindow) {
    let dmi = DMIID::new().unwrap_or_default();
    let cpu_name = read_cpu_name();
    let gpu_name = read_gpu_name();
    let ram_info = read_ram_total();

    let product = ProductInfo {
        product_name: dmi.product_name.into(),
        cpu_name: cpu_name.into(),
        gpu_name: gpu_name.into(),
        ram_info: ram_info.into(),
        bios_version: format!("BIOS {}", dmi.bios_version).into(),
    };

    let data = ui.global::<DashboardPageData>();
    data.set_product(product);

    // Set initial fan modes
    let fan_modes = vec![
        FanModeEntry {
            label: "Balanced".into(),
            active: true,
        },
        FanModeEntry {
            label: "Performance".into(),
            active: false,
        },
        FanModeEntry {
            label: "Quiet".into(),
            active: false,
        },
    ];
    data.set_fan_modes(ModelRc::new(VecModel::from(fan_modes)));

    // Fan mode callback — will be wired to D-Bus in a future step
    data.on_cb_set_fan_mode(|idx| {
        log::info!("Fan mode selected: {idx}");
    });
}

/// Start a background task that polls sysfs/procfs every 2 seconds.
pub fn setup_dashboard_monitoring(handle: Weak<MainWindow>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        // Discover hwmon devices once
        let hwmon_map = discover_hwmon_devices();

        // For CPU usage delta calculation
        let mut prev_cpu_stats: Option<(u64, u64)> = None;

        loop {
            interval.tick().await;

            let freq = read_cpu_frequencies();
            let temp = read_temperatures(&hwmon_map);
            let (usage, new_cpu_stats) = read_usage_stats(prev_cpu_stats);
            prev_cpu_stats = Some(new_cpu_stats);
            let fan = read_fan_speeds(&hwmon_map);
            let voltage = read_voltages(&hwmon_map);

            let handle_clone = handle.clone();
            handle_clone
                .upgrade_in_event_loop(move |ui| {
                    let data = ui.global::<DashboardPageData>();
                    data.set_frequency_entries(ModelRc::new(VecModel::from(freq)));
                    data.set_temperature_entries(ModelRc::new(VecModel::from(temp)));
                    data.set_usage_entries(ModelRc::new(VecModel::from(usage)));
                    data.set_fan_entries(ModelRc::new(VecModel::from(fan)));
                    data.set_voltage_entries(ModelRc::new(VecModel::from(voltage)));
                })
                .ok();
        }
    });
}

// ── Helper: read CPU model name ──

fn read_cpu_name() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default()
}

// ── Helper: read GPU name from DRM sysfs ──

fn read_gpu_name() -> String {
    // Try reading from DRM device sysfs
    let drm_path = "/sys/class/drm";
    if let Ok(entries) = fs::read_dir(drm_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("card") && !name.contains('-') {
                let vendor_path = entry.path().join("device/vendor");
                let device_path = entry.path().join("device/device");
                if let (Ok(vendor), Ok(device)) = (
                    fs::read_to_string(&vendor_path),
                    fs::read_to_string(&device_path),
                ) {
                    let vendor = vendor.trim();
                    let device = device.trim();
                    let vendor_name = match vendor {
                        "0x1002" => "AMD",
                        "0x10de" => "NVIDIA",
                        "0x8086" => "Intel",
                        _ => continue,
                    };
                    return format!("{vendor_name} GPU ({device})");
                }
            }
        }
    }
    String::new()
}

// ── Helper: read total RAM ──

fn read_ram_total() -> String {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("MemTotal:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<u64>().ok())
                .map(|kb| {
                    let gb = (kb as f64) / 1024.0 / 1024.0;
                    format!("{:.0} GB RAM", gb.ceil())
                })
        })
        .unwrap_or_default()
}

// ── Hwmon device discovery ──

fn discover_hwmon_devices() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let hwmon_dir = "/sys/class/hwmon";
    if let Ok(entries) = fs::read_dir(hwmon_dir) {
        for entry in entries.flatten() {
            let name_path = entry.path().join("name");
            if let Ok(name) = fs::read_to_string(&name_path) {
                let name = name.trim().to_string();
                let path = entry.path().to_string_lossy().to_string();
                map.insert(name, path);
            }
        }
    }
    map
}

// ── CPU frequencies ──

fn read_cpu_frequencies() -> Vec<MonitorEntry> {
    let mut entries = Vec::new();
    let cpu_dir = "/sys/devices/system/cpu";

    // Read per-core frequencies
    let mut core_freqs = Vec::new();
    for i in 0..32 {
        let path = format!("{cpu_dir}/cpu{i}/cpufreq/scaling_cur_freq");
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(khz) = content.trim().parse::<u64>() {
                core_freqs.push(khz);
            }
        } else {
            break;
        }
    }

    if !core_freqs.is_empty() {
        // Average CPU frequency
        let avg_mhz = core_freqs.iter().sum::<u64>() / core_freqs.len() as u64 / 1000;
        let max_mhz = *core_freqs.iter().max().unwrap_or(&0) / 1000;
        entries.push(MonitorEntry {
            label: "CPU".into(),
            value: format!("{avg_mhz} MHz").into(),
            bar_percent: (avg_mhz as f32 / 6000.0).min(1.0), // assume 6GHz max
            show_bar: true,
        });

        // Show individual cores (up to 8)
        for (i, freq) in core_freqs.iter().take(8).enumerate() {
            let mhz = freq / 1000;
            entries.push(MonitorEntry {
                label: format!("CPU Core {i}").into(),
                value: format!("{mhz} MHz").into(),
                bar_percent: (mhz as f32 / max_mhz.max(1) as f32).min(1.0),
                show_bar: false,
            });
        }
    }

    entries
}

// ── Temperatures ──

fn read_temperatures(hwmon_map: &HashMap<String, String>) -> Vec<MonitorEntry> {
    let mut entries = Vec::new();

    // CPU temperature - try k10temp (AMD) or coretemp (Intel)
    let cpu_hwmon = hwmon_map
        .get("k10temp")
        .or_else(|| hwmon_map.get("coretemp"));

    if let Some(path) = cpu_hwmon {
        if let Some(temp) = read_hwmon_temp(path, "temp1_input") {
            entries.push(MonitorEntry {
                label: "CPU".into(),
                value: format!("{temp} \u{00B0}C").into(),
                bar_percent: (temp as f32 / 105.0).min(1.0),
                show_bar: true,
            });
        }
        // CPU Package / Tctl
        if let Some(temp) = read_hwmon_temp(path, "temp2_input") {
            entries.push(MonitorEntry {
                label: "CPU Package".into(),
                value: format!("{temp} \u{00B0}C").into(),
                bar_percent: (temp as f32 / 105.0).min(1.0),
                show_bar: true,
            });
        }
    }

    // GPU temperature
    let gpu_hwmon = hwmon_map.get("amdgpu").or_else(|| hwmon_map.get("nvidia"));
    if let Some(path) = gpu_hwmon {
        if let Some(temp) = read_hwmon_temp(path, "temp1_input") {
            entries.push(MonitorEntry {
                label: "GPU".into(),
                value: format!("{temp} \u{00B0}C").into(),
                bar_percent: (temp as f32 / 105.0).min(1.0),
                show_bar: false,
            });
        }
    }

    // ASUS-specific sensors
    for name in ["asus-ec-sensors", "asus_wmi_sensors", "asus-isa-sensors"] {
        if let Some(path) = hwmon_map.get(name) {
            // Try reading available temperature inputs
            for i in 1..=6 {
                let input = format!("temp{i}_input");
                let label_file = format!("temp{i}_label");
                if let Some(temp) = read_hwmon_temp(path, &input) {
                    let label = fs::read_to_string(format!("{path}/{label_file}"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| format!("Sensor {i}"));
                    entries.push(MonitorEntry {
                        label: label.into(),
                        value: format!("{temp} \u{00B0}C").into(),
                        bar_percent: (temp as f32 / 105.0).min(1.0),
                        show_bar: false,
                    });
                }
            }
        }
    }

    entries
}

fn read_hwmon_temp(hwmon_path: &str, file: &str) -> Option<u64> {
    let path = format!("{hwmon_path}/{file}");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|millideg| millideg / 1000)
}

// ── CPU & RAM usage ──

fn read_usage_stats(
    prev_cpu: Option<(u64, u64)>,
) -> (Vec<MonitorEntry>, (u64, u64)) {
    let mut entries = Vec::new();

    // CPU usage from /proc/stat
    let (cpu_percent, total, idle) = read_cpu_usage(prev_cpu);
    entries.push(MonitorEntry {
        label: "CPU (Average) Usage".into(),
        value: format!("{cpu_percent} %").into(),
        bar_percent: cpu_percent as f32 / 100.0,
        show_bar: false,
    });

    // RAM usage from /proc/meminfo
    if let Some((used_percent, used_gb, total_gb)) = read_ram_usage() {
        entries.push(MonitorEntry {
            label: "RAM Usage".into(),
            value: format!("{used_gb:.1} / {total_gb:.1} GB ({used_percent}%)").into(),
            bar_percent: used_percent as f32 / 100.0,
            show_bar: false,
        });
    }

    (entries, (total, idle))
}

fn read_cpu_usage(prev: Option<(u64, u64)>) -> (u64, u64, u64) {
    let content = match fs::read_to_string("/proc/stat") {
        Ok(c) => c,
        Err(_) => return (0, 0, 0),
    };

    let first_line = match content.lines().next() {
        Some(l) if l.starts_with("cpu ") => l,
        _ => return (0, 0, 0),
    };

    let vals: Vec<u64> = first_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();

    if vals.len() < 4 {
        return (0, 0, 0);
    }

    let total: u64 = vals.iter().sum();
    let idle = vals[3]; // idle is the 4th value

    let percent = if let Some((prev_total, prev_idle)) = prev {
        let d_total = total.saturating_sub(prev_total);
        let d_idle = idle.saturating_sub(prev_idle);
        if d_total > 0 {
            ((d_total - d_idle) * 100 / d_total).min(100)
        } else {
            0
        }
    } else {
        0
    };

    (percent, total, idle)
}

fn read_ram_usage() -> Option<(u64, f64, f64)> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut mem_total = 0u64;
    let mut mem_available = 0u64;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            mem_total = line.split_whitespace().nth(1)?.parse().ok()?;
        } else if line.starts_with("MemAvailable:") {
            mem_available = line.split_whitespace().nth(1)?.parse().ok()?;
        }
    }

    if mem_total == 0 {
        return None;
    }

    let used = mem_total - mem_available;
    let percent = (used * 100) / mem_total;
    let total_gb = mem_total as f64 / 1024.0 / 1024.0;
    let used_gb = used as f64 / 1024.0 / 1024.0;

    Some((percent, used_gb, total_gb))
}

// ── Fan speeds ──

fn read_fan_speeds(hwmon_map: &HashMap<String, String>) -> Vec<MonitorEntry> {
    let mut entries = Vec::new();

    // ASUS WMI fan sensors
    for name in ["asus-nb-wmi", "asus_wmi_sensors", "asus-ec-sensors"] {
        if let Some(path) = hwmon_map.get(name) {
            for i in 1..=8 {
                let input = format!("fan{i}_input");
                let label_file = format!("fan{i}_label");
                let full_path = format!("{path}/{input}");
                if let Ok(content) = fs::read_to_string(&full_path) {
                    if let Ok(rpm) = content.trim().parse::<u64>() {
                        let label = fs::read_to_string(format!("{path}/{label_file}"))
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|_| format!("Fan {i}"));
                        entries.push(MonitorEntry {
                            label: label.into(),
                            value: format!("{rpm} RPM").into(),
                            bar_percent: (rpm as f32 / 6000.0).min(1.0),
                            show_bar: false,
                        });
                    }
                }
            }
        }
    }

    // If no ASUS-specific fans found, try generic hwmon fans
    if entries.is_empty() {
        for (name, path) in hwmon_map {
            for i in 1..=4 {
                let input = format!("fan{i}_input");
                let full_path = format!("{path}/{input}");
                if let Ok(content) = fs::read_to_string(&full_path) {
                    if let Ok(rpm) = content.trim().parse::<u64>() {
                        entries.push(MonitorEntry {
                            label: format!("{name} Fan {i}").into(),
                            value: format!("{rpm} RPM").into(),
                            bar_percent: (rpm as f32 / 6000.0).min(1.0),
                            show_bar: false,
                        });
                    }
                }
            }
        }
    }

    entries
}

// ── Voltages (if available) ──

fn read_voltages(hwmon_map: &HashMap<String, String>) -> Vec<MonitorEntry> {
    let mut entries = Vec::new();

    for name in ["asus-ec-sensors", "asus_wmi_sensors", "asus-isa-sensors"] {
        if let Some(path) = hwmon_map.get(name) {
            for i in 0..=8 {
                let input = format!("in{i}_input");
                let label_file = format!("in{i}_label");
                let full_path = format!("{path}/{input}");
                if let Ok(content) = fs::read_to_string(&full_path) {
                    if let Ok(mv) = content.trim().parse::<u64>() {
                        let label = fs::read_to_string(format!("{path}/{label_file}"))
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|_| format!("Voltage {i}"));
                        let volts = mv as f64 / 1000.0;
                        entries.push(MonitorEntry {
                            label: label.into(),
                            value: format!("{volts:.3} V").into(),
                            bar_percent: 0.0,
                            show_bar: false,
                        });
                    }
                }
            }
        }
    }

    entries
}
