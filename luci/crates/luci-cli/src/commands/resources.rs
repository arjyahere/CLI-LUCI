use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_banner();

    let response = client.send(IpcCommand::GetResources)?;

    if let Some(payload) = response.payload() {
        let devices = payload["devices"].as_u64().unwrap_or(0);
        let cpu_cores = payload["cpu_cores"].as_u64().unwrap_or(0);
        let ram_total = payload["ram_total_gb"].as_str().unwrap_or("0.0").parse::<f64>().unwrap_or(0.0);
        let ram_avail = payload["ram_available_gb"].as_str().unwrap_or("0.0").parse::<f64>().unwrap_or(0.0);
        let gpu_count = payload["gpu_count"].as_u64().unwrap_or(0);
        let vram_total = payload["vram_total_gb"].as_str().unwrap_or("0.0").parse::<f64>().unwrap_or(0.0);

        println!("  {}{}╔══════════════════════════════════════════════════════════════╗{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}║                   CLUSTER RESOURCES                          ║{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}╚══════════════════════════════════════════════════════════════╝{}", display::DIM, display::BOLD, display::RESET);
        println!();

        // Overview
        println!("  {}{}─── Overview ───────────────────────────────────────────────{}", display::DIM, display::BOLD, display::RESET);
        println!();
        display::print_kv("Devices", &format!("{} connected", devices));
        println!();

        // CPU
        println!("  {}{}─── CPU ────────────────────────────────────────────────────{}", display::DIM, display::BOLD, display::RESET);
        println!();
        display::print_kv("Total Cores", &cpu_cores.to_string());
        let cpu_bar = display::progress_bar(cpu_cores as f64, 128.0, 40);
        println!("  {}Capacity:{}      [{}]", display::DIM, display::RESET, cpu_bar);
        println!();

        // Memory
        println!("  {}{}─── Memory ─────────────────────────────────────────────────{}", display::DIM, display::BOLD, display::RESET);
        println!();
        display::print_kv("Total", &format!("{:.1} GB", ram_total));
        display::print_kv_status("Available", &format!("{:.1} GB", ram_avail), ram_avail > ram_total * 0.25);
        let ram_used = ram_total - ram_avail;
        let ram_pct = if ram_total > 0.0 { (ram_used / ram_total) * 100.0 } else { 0.0 };
        let ram_bar = display::progress_bar(ram_used, ram_total, 40);
        println!("  {}Usage:{}          [{}] {:.1}%", display::DIM, display::RESET, ram_bar, ram_pct);
        println!();

        // GPU
        println!("  {}{}─── GPU ────────────────────────────────────────────────────{}", display::DIM, display::BOLD, display::RESET);
        println!();
        if gpu_count > 0 {
            display::print_kv("GPU Devices", &format!("{} device(s)", gpu_count));
            display::print_kv("Total VRAM", &format!("{:.1} GB", vram_total));
            let vram_bar = display::progress_bar(vram_total, 96.0, 40);
            println!("  {}VRAM Usage:{}     [{}]", display::DIM, display::RESET, vram_bar);
        } else {
            println!("  {}No GPU devices detected{}", display::DIM, display::RESET);
        }
        println!();

        // Capacity summary
        let capacity = if ram_total > 0.0 { ((ram_avail / ram_total) * 100.0) as u64 } else { 0 };
        let capacity_status = if capacity > 50 { "Healthy" } else if capacity > 25 { "Moderate" } else { "Low" };
        println!("  {}{}─── Cluster Capacity ──────────────────────────────────────{}", display::DIM, display::BOLD, display::RESET);
        println!();
        println!("  {}Status:{}        {}{} {}{}", display::DIM, display::RESET, display::status_indicator(capacity_status), display::BRIGHT_GREEN, capacity_status, display::RESET);
        display::print_kv_status("Available", &format!("{}%", capacity), capacity > 25);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
