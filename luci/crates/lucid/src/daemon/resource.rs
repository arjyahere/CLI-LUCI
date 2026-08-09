use chrono::Utc;
use luci_common::{CpuInfo, DiskInfo, GpuInfo, MemoryInfo, NetworkInfo, NodeId, ResourceSnapshot};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sysinfo::{Disks, Networks, System};

#[derive(Clone)]
pub struct ResourceCollector {
    node_id: NodeId,
    idle_seconds: Arc<AtomicU64>,
}

impl ResourceCollector {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            idle_seconds: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn collect(&self) -> ResourceSnapshot {
        let mut sys = System::new_all();
        sys.refresh_all();

        let cpu = self.collect_cpu(&sys);
        let memory = self.collect_memory(&sys);
        let gpu = self.collect_gpu().await;
        let disk = self.collect_disk();
        let network = self.collect_network();
        let (is_charging, battery_percent) = self.collect_battery();
        let temperature_c = self.collect_temperature().await;

        // Update idle_seconds: if CPU usage < 5%, increment by collection interval (5s)
        if cpu.load_percent < 5.0 {
            self.idle_seconds.fetch_add(5, Ordering::Relaxed);
        } else {
            self.idle_seconds.store(0, Ordering::Relaxed);
        }
        let idle_seconds = self.idle_seconds.load(Ordering::Relaxed);

        ResourceSnapshot {
            node_id: self.node_id.clone(),
            timestamp: Utc::now(),
            cpu,
            memory,
            gpu,
            disk,
            network,
            idle_seconds,
            is_charging,
            battery_percent,
            temperature_c,
        }
    }

    fn collect_cpu(&self, sys: &System) -> CpuInfo {
        let cores = sys.cpus().len();
        let load = sys.global_cpu_usage();
        let frequency = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

        CpuInfo {
            cores,
            load_percent: load,
            frequency_mhz: frequency,
        }
    }

    fn collect_memory(&self, sys: &System) -> MemoryInfo {
        let total = sys.total_memory();
        let available = sys.available_memory();
        let used = total - available;

        MemoryInfo {
            total_bytes: total,
            available_bytes: available,
            used_bytes: used,
        }
    }

    async fn collect_gpu(&self) -> Option<GpuInfo> {
        // Try to get GPU info from nvidia-smi (works on Windows, Linux, and macOS with NVIDIA drivers)
        if let Ok(output) = tokio::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,memory.used,memory.free,utilization.gpu,temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let parts: Vec<&str> = line.split(", ").collect();
                if parts.len() >= 6 {
                    let name = parts[0].trim().to_string();
                    let vram_total = parts[1].trim().parse::<u64>().unwrap_or(0) * 1024 * 1024;
                    let vram_used = parts[2].trim().parse::<u64>().unwrap_or(0) * 1024 * 1024;
                    let vram_free = parts[3].trim().parse::<u64>().unwrap_or(0) * 1024 * 1024;
                    let utilization = parts[4].trim().parse::<f32>().unwrap_or(0.0);
                    let temperature = parts[5].trim().parse::<f32>().ok();

                    return Some(GpuInfo {
                        name,
                        vram_total_bytes: vram_total,
                        vram_used_bytes: vram_used,
                        vram_available_bytes: vram_free,
                        utilization_percent: utilization,
                        temperature_c: temperature,
                    });
                }
            }
        }

        None
    }

    fn collect_disk(&self) -> DiskInfo {
        let disks = Disks::new_with_refreshed_list();
        let mut total = 0u64;
        let mut free = 0u64;

        for disk in &disks {
            total += disk.total_space();
            free += disk.available_space();
        }

        DiskInfo {
            total_bytes: total,
            free_bytes: free,
        }
    }

    fn collect_network(&self) -> NetworkInfo {
        let networks = Networks::new_with_refreshed_list();
        let mut bytes_sent = 0u64;
        let mut bytes_received = 0u64;

        for (_, data) in &networks {
            bytes_sent += data.total_transmitted();
            bytes_received += data.total_received();
        }

        NetworkInfo {
            bytes_sent,
            bytes_received,
        }
    }

    fn collect_battery(&self) -> (Option<bool>, Option<f32>) {
        // Try to get battery info from platform-specific commands
        #[cfg(target_os = "windows")]
        {
            // On Windows, we can use WMIC or PowerShell
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-Command", "Get-WmiObject -Class Win32_Battery | Select-Object BatteryStatus, EstimatedChargeRemaining"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse output - this is a simplified parser
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 1 {
                    let parts: Vec<&str> = lines[1].split_whitespace().collect();
                    if parts.len() >= 2 {
                        let is_charging = parts[0].parse::<u32>().ok().map(|s| s == 2 || s == 6 || s == 7 || s == 8);
                        let battery_percent = parts[1].parse::<f32>().ok();
                        return (is_charging, battery_percent);
                    }
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // On Linux, read from /sys/class/power_supply/
            if let Ok(capacity) = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
                if let Ok(percent) = capacity.trim().parse::<f32>() {
                    let status = std::fs::read_to_string("/sys/class/power_supply/BAT0/status")
                        .unwrap_or_default();
                    let is_charging = status.trim() == "Charging";
                    return (Some(is_charging), Some(percent));
                }
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            // On macOS, use pmset
            if let Ok(output) = std::process::Command::new("pmset")
                .arg("-g")
                .arg("batt")
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 1 {
                    let is_charging = lines[1].contains("charging") || lines[1].contains("AC attached");
                    let percent = lines[1]
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.trim_end_matches('%').parse::<f32>().ok());
                    return (Some(is_charging), percent);
                }
            }
        }
        
        (None, None)
    }

    async fn collect_temperature(&self) -> Option<f32> {
        // Try to get temperature from various sources
        #[cfg(target_os = "linux")]
        {
            // Try thermal zone
            if let Ok(temp) = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
                if let Ok(millidegrees) = temp.trim().parse::<f32>() {
                    return Some(millidegrees / 1000.0);
                }
            }
        }
        
        // Try nvidia-smi for GPU temperature
        if let Ok(output) = tokio::process::Command::new("nvidia-smi")
            .arg("--query-gpu=temperature.gpu")
            .arg("--format=csv,noheader,nounits")
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(temp) = stdout.trim().parse::<f32>() {
                return Some(temp);
            }
        }
        
        None
    }
}
