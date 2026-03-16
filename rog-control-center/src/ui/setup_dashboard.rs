use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

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

    // Detect GPU availability
    let hwmon_map = discover_hwmon_devices();
    let has_gpu = hwmon_map.contains_key("amdgpu") || hwmon_map.contains_key("nvidia");
    data.set_gpu_available(has_gpu);

    // Operation mode (Silence / Performance / Turbo)
    let fan_modes = vec![
        FanModeEntry {
            label: "Silence".into(),
            active: false,
        },
        FanModeEntry {
            label: "Performance".into(),
            active: true,
        },
        FanModeEntry {
            label: "Turbo".into(),
            active: false,
        },
    ];
    data.set_fan_modes(ModelRc::new(VecModel::from(fan_modes)));

    // Fan mode callback — will be wired to D-Bus in a future step
    data.on_cb_set_fan_mode(|idx| {
        log::info!("Operation mode selected: {idx}");
    });
}

/// Start a background task that polls sysfs/procfs every 2 seconds.
pub fn setup_dashboard_monitoring(handle: Weak<MainWindow>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        let hwmon_map = discover_hwmon_devices();
        let drm_device = find_gpu_drm_device(&hwmon_map);

        let mut prev_cpu_stats: Option<(u64, u64)> = None;
        let mut prev_rapl: Option<(u64, Instant)> = None;

        loop {
            interval.tick().await;

            let (cpu, new_cpu_stats, new_rapl) =
                build_cpu_entries(&hwmon_map, prev_cpu_stats, prev_rapl);
            prev_cpu_stats = Some(new_cpu_stats);
            prev_rapl = new_rapl;

            let gpu = build_gpu_entries(&hwmon_map, &drm_device);
            let fan = build_fan_entries(&hwmon_map);
            let storage = build_storage_entries();

            let h = handle.clone();
            h.upgrade_in_event_loop(move |ui| {
                let data = ui.global::<DashboardPageData>();
                data.set_cpu_entries(ModelRc::new(VecModel::from(cpu)));
                data.set_gpu_entries(ModelRc::new(VecModel::from(gpu)));
                data.set_fan_entries(ModelRc::new(VecModel::from(fan)));
                data.set_storage_entries(ModelRc::new(VecModel::from(storage)));
            })
            .ok();
        }
    });
}

// ═══════════════════════════════════════════════════════════════════
// Device-centric composition functions
// ═══════════════════════════════════════════════════════════════════

/// Compose CPU monitoring entries: Frequency, Power, Memory, Temperature, Voltage.
fn build_cpu_entries(
    hwmon_map: &HashMap<String, String>,
    prev_cpu: Option<(u64, u64)>,
    prev_rapl: Option<(u64, Instant)>,
) -> (Vec<MonitorEntry>, (u64, u64), Option<(u64, Instant)>) {
    let mut entries = Vec::new();

    // 1. CPU Frequency (average, progress bar)
    let mut core_freqs = Vec::new();
    for i in 0..128 {
        let path = format!("/sys/devices/system/cpu/cpu{i}/cpufreq/scaling_cur_freq");
        match fs::read_to_string(&path) {
            Ok(s) => {
                if let Ok(khz) = s.trim().parse::<u64>() {
                    core_freqs.push(khz);
                }
            }
            Err(_) => break,
        }
    }
    if !core_freqs.is_empty() {
        let avg_mhz = core_freqs.iter().sum::<u64>() / core_freqs.len() as u64 / 1000;
        let max_mhz = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|khz| khz / 1000)
            .unwrap_or(6000);
        entries.push(MonitorEntry {
            label: "Frequency".into(),
            value: format!("{avg_mhz} MHz").into(),
            bar_percent: (avg_mhz as f32 / max_mhz as f32).min(1.0),
            show_bar: true,
        });
    }

    // 2. CPU Power (RAPL, progress bar)
    let (watts, new_rapl) = read_cpu_power(prev_rapl);
    if let Some(w) = watts {
        entries.push(MonitorEntry {
            label: "CPU Power".into(),
            value: format!("{w:.1} W").into(),
            bar_percent: (w as f32 / 65.0).min(1.0),
            show_bar: true,
        });
    }

    // 3. Memory Usage %
    if let Some((percent, _used_gb, _total_gb)) = read_ram_usage() {
        entries.push(MonitorEntry {
            label: "Memory".into(),
            value: format!("{percent} %").into(),
            bar_percent: 0.0,
            show_bar: false,
        });
    }

    // 4. CPU Temperature
    let cpu_hwmon = hwmon_map
        .get("k10temp")
        .or_else(|| hwmon_map.get("coretemp"));
    if let Some(path) = cpu_hwmon {
        if let Some(temp) = read_hwmon_temp(path, "temp1_input") {
            entries.push(MonitorEntry {
                label: "Temperature".into(),
                value: format!("{temp} \u{00B0}C").into(),
                bar_percent: 0.0,
                show_bar: false,
            });
        }
    }

    // 5. CPU Voltage (from ASUS EC sensors if available)
    for name in ["asus-ec-sensors", "asus_wmi_sensors"] {
        if let Some(path) = hwmon_map.get(name) {
            let full_path = format!("{path}/in0_input");
            if let Ok(content) = fs::read_to_string(&full_path) {
                if let Ok(mv) = content.trim().parse::<u64>() {
                    let volts = mv as f64 / 1000.0;
                    entries.push(MonitorEntry {
                        label: "Voltage".into(),
                        value: format!("{volts:.3} V").into(),
                        bar_percent: 0.0,
                        show_bar: false,
                    });
                    break;
                }
            }
        }
    }

    // Track CPU usage delta (used for cpu_percent calculation)
    let (_cpu_percent, total, idle) = read_cpu_usage(prev_cpu);

    (entries, (total, idle), new_rapl)
}

/// Compose GPU monitoring entries: Frequency, Power, Memory, Mem Freq, Temp, Voltage.
fn build_gpu_entries(
    hwmon_map: &HashMap<String, String>,
    drm_device: &Option<String>,
) -> Vec<MonitorEntry> {
    let mut entries = Vec::new();

    // 1. GPU Frequency (progress bar)
    if let Some(mhz) = read_gpu_frequency(hwmon_map) {
        entries.push(MonitorEntry {
            label: "Frequency".into(),
            value: format!("{mhz} MHz").into(),
            bar_percent: (mhz as f32 / 2500.0).min(1.0),
            show_bar: true,
        });
    }

    // 2. GPU Power (progress bar)
    if let Some(watts) = read_gpu_power(hwmon_map) {
        entries.push(MonitorEntry {
            label: "GPU Power".into(),
            value: format!("{watts:.1} W").into(),
            bar_percent: (watts as f32 / 150.0).min(1.0),
            show_bar: true,
        });
    }

    // 3. VRAM Usage
    if let Some(drm) = drm_device {
        if let Some((used_mb, total_mb)) = read_gpu_vram(drm) {
            let percent = if total_mb > 0 {
                used_mb as f32 / total_mb as f32
            } else {
                0.0
            };
            entries.push(MonitorEntry {
                label: "Memory".into(),
                value: format!("{used_mb} / {total_mb} MB").into(),
                bar_percent: percent,
                show_bar: false,
            });
        }

        // 4. GPU Memory Frequency
        if let Some(mhz) = read_gpu_mem_frequency(drm) {
            entries.push(MonitorEntry {
                label: "Memory Frequency".into(),
                value: format!("{mhz} MHz").into(),
                bar_percent: 0.0,
                show_bar: false,
            });
        }
    }

    // 5. GPU Temperature
    let gpu_hwmon = hwmon_map.get("amdgpu").or_else(|| hwmon_map.get("nvidia"));
    if let Some(path) = gpu_hwmon {
        if let Some(temp) = read_hwmon_temp(path, "temp1_input") {
            entries.push(MonitorEntry {
                label: "Temperature".into(),
                value: format!("{temp} \u{00B0}C").into(),
                bar_percent: 0.0,
                show_bar: false,
            });
        }
    }

    // 6. GPU Voltage
    if let Some(mv) = read_gpu_voltage(hwmon_map) {
        entries.push(MonitorEntry {
            label: "Voltage".into(),
            value: format!("{mv} mV").into(),
            bar_percent: 0.0,
            show_bar: false,
        });
    }

    entries
}

/// Compose Fan monitoring entries: CPU Fan, GPU Fan (all with progress bars).
fn build_fan_entries(hwmon_map: &HashMap<String, String>) -> Vec<MonitorEntry> {
    let mut entries = Vec::new();

    // Try ASUS-specific fan sensors first
    for name in ["asus-nb-wmi", "asus_wmi_sensors", "asus-ec-sensors"] {
        if let Some(path) = hwmon_map.get(name) {
            for i in 1..=8 {
                let input = format!("{path}/fan{i}_input");
                let label_file = format!("{path}/fan{i}_label");
                if let Ok(content) = fs::read_to_string(&input) {
                    if let Ok(rpm) = content.trim().parse::<u64>() {
                        let label = fs::read_to_string(&label_file)
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|_| format!("Fan {i}"));
                        entries.push(MonitorEntry {
                            label: label.into(),
                            value: format!("{rpm} RPM").into(),
                            bar_percent: (rpm as f32 / 5000.0).min(1.0),
                            show_bar: true,
                        });
                    }
                }
            }
        }
    }

    // Fallback: generic hwmon fans
    if entries.is_empty() {
        for (_name, path) in hwmon_map {
            for i in 1..=4 {
                let input = format!("{path}/fan{i}_input");
                if let Ok(content) = fs::read_to_string(&input) {
                    if let Ok(rpm) = content.trim().parse::<u64>() {
                        let label = if i == 1 { "CPU Fan" } else { "GPU Fan" };
                        entries.push(MonitorEntry {
                            label: label.into(),
                            value: format!("{rpm} RPM").into(),
                            bar_percent: (rpm as f32 / 5000.0).min(1.0),
                            show_bar: true,
                        });
                    }
                }
            }
        }
    }

    entries
}

/// Compose Storage monitoring entries: Disk usage + RAM (all with progress bars).
fn build_storage_entries() -> Vec<MonitorEntry> {
    let mut entries = Vec::new();

    // 1. Root filesystem
    if let Some((used_gb, total_gb, percent)) = read_disk_usage() {
        entries.push(MonitorEntry {
            label: "Storage".into(),
            value: format!("{used_gb:.1} / {total_gb:.1} GB").into(),
            bar_percent: percent as f32 / 100.0,
            show_bar: true,
        });
    }

    // 2. RAM
    if let Some((percent, used_gb, total_gb)) = read_ram_usage() {
        entries.push(MonitorEntry {
            label: "RAM".into(),
            value: format!("{used_gb:.1} / {total_gb:.1} GB").into(),
            bar_percent: percent as f32 / 100.0,
            show_bar: true,
        });
    }

    entries
}

// ═══════════════════════════════════════════════════════════════════
// Low-level data readers
// ═══════════════════════════════════════════════════════════════════

// ── CPU info ──

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
    let idle = vals[3];

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

/// Read CPU package power from RAPL energy_uj (delta-based).
fn read_cpu_power(prev: Option<(u64, Instant)>) -> (Option<f64>, Option<(u64, Instant)>) {
    let energy = fs::read_to_string("/sys/class/powercap/intel-rapl:0/energy_uj")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());

    let now = Instant::now();
    match (energy, prev) {
        (Some(curr_uj), Some((prev_uj, prev_time))) => {
            let dt = now.duration_since(prev_time).as_secs_f64();
            if dt > 0.1 {
                let delta = curr_uj.wrapping_sub(prev_uj);
                let watts = (delta as f64) / 1_000_000.0 / dt;
                (Some(watts), Some((curr_uj, now)))
            } else {
                (None, Some((curr_uj, now)))
            }
        }
        (Some(curr_uj), None) => (None, Some((curr_uj, now))),
        _ => (None, None),
    }
}

// ── GPU info ──

fn read_gpu_name() -> String {
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

/// Find the DRM device path for the discrete GPU (amdgpu or nvidia).
fn find_gpu_drm_device(hwmon_map: &HashMap<String, String>) -> Option<String> {
    let gpu_hwmon = hwmon_map.get("amdgpu").or_else(|| hwmon_map.get("nvidia"))?;
    let hwmon_device = fs::canonicalize(format!("{gpu_hwmon}/device")).ok()?;

    for entry in fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("card") && !name.contains('-') {
            if let Ok(drm_device) = fs::canonicalize(entry.path().join("device")) {
                if drm_device == hwmon_device {
                    return Some(drm_device.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

/// Read GPU core frequency from amdgpu hwmon freq1_input (Hz -> MHz).
fn read_gpu_frequency(hwmon_map: &HashMap<String, String>) -> Option<u64> {
    let path = hwmon_map.get("amdgpu")?;
    let hz = fs::read_to_string(format!("{path}/freq1_input"))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(hz / 1_000_000)
}

/// Read GPU power from amdgpu hwmon power1_average or power1_input (microwatts -> W).
fn read_gpu_power(hwmon_map: &HashMap<String, String>) -> Option<f64> {
    let path = hwmon_map.get("amdgpu")?;
    let uw = fs::read_to_string(format!("{path}/power1_average"))
        .or_else(|_| fs::read_to_string(format!("{path}/power1_input")))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(uw as f64 / 1_000_000.0)
}

/// Read GPU VRAM usage from DRM sysfs (bytes -> MB).
fn read_gpu_vram(drm_device: &str) -> Option<(u64, u64)> {
    let used = fs::read_to_string(format!("{drm_device}/mem_info_vram_used"))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?
        / (1024 * 1024);
    let total = fs::read_to_string(format!("{drm_device}/mem_info_vram_total"))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?
        / (1024 * 1024);
    Some((used, total))
}

/// Read GPU memory frequency from pp_dpm_mclk (active line ends with *).
fn read_gpu_mem_frequency(drm_device: &str) -> Option<u64> {
    let content = fs::read_to_string(format!("{drm_device}/pp_dpm_mclk")).ok()?;
    for line in content.lines() {
        if line.trim_end().ends_with('*') {
            return line
                .split_whitespace()
                .nth(1)?
                .trim_end_matches("Mhz")
                .trim_end_matches("MHz")
                .parse::<u64>()
                .ok();
        }
    }
    None
}

/// Read GPU voltage from amdgpu hwmon in0_input (millivolts).
fn read_gpu_voltage(hwmon_map: &HashMap<String, String>) -> Option<u64> {
    let path = hwmon_map.get("amdgpu")?;
    fs::read_to_string(format!("{path}/in0_input"))
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

// ── RAM info ──

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

// ── Disk usage ──

/// Read root filesystem usage via statvfs64.
fn read_disk_usage() -> Option<(f64, f64, u64)> {
    use std::ffi::CString;
    let path = CString::new("/").ok()?;
    let mut stat: libc::statvfs64 = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs64(path.as_ptr(), &mut stat) };
    if ret != 0 {
        return None;
    }

    let block_size = stat.f_frsize as u64;
    let total = stat.f_blocks * block_size;
    let free = stat.f_bfree * block_size;
    let used = total - free;

    let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
    let used_gb = used as f64 / 1024.0 / 1024.0 / 1024.0;
    let percent = if total > 0 { (used * 100) / total } else { 0 };

    Some((used_gb, total_gb, percent))
}

// ── Hwmon helpers ──

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

fn read_hwmon_temp(hwmon_path: &str, file: &str) -> Option<u64> {
    let path = format!("{hwmon_path}/{file}");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|millideg| millideg / 1000)
}
