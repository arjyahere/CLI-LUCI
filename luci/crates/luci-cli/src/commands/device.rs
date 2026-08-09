use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, name: &str) -> Result<()> {
    display::print_header("DEVICE DETAILS");

    let response = client.send(IpcCommand::GetDevice {
        name: name.to_string(),
    })?;

    if let Some(payload) = response.payload() {
        let device_name = payload["name"].as_str().unwrap_or("unknown");
        let status = payload["status"].as_str().unwrap_or("unknown");
        let cpu_cores = payload["cpu_cores"].as_u64().unwrap_or(0);
        let cpu_load = payload["cpu_load"].as_f64().unwrap_or(0.0);
        let ram_total = payload["ram_total_gb"].as_f64().unwrap_or(0.0);
        let ram_avail = payload["ram_available_gb"].as_f64().unwrap_or(0.0);
        let gpu_name = payload["gpu_name"].as_str().unwrap_or("-");

        let status_display = display::format_node_status(status);

        println!("  {}Name:{}       {}{}{}", display::DIM, display::RESET, display::BRIGHT_CYAN, device_name, display::RESET);
        println!("  {}Status:{}     {}", display::DIM, display::RESET, status_display);
        println!();
        
        println!("  {}{}─── Resources ──────────────────────────────────────────────{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}CPU:{}         {}{} cores (load: {:.1}%){}", display::DIM, display::RESET, display::WHITE, cpu_cores, cpu_load, display::RESET);
        println!("  {}RAM:{}         {}{:.1} GB total, {:.1} GB free{}", display::DIM, display::RESET, display::WHITE, ram_total, ram_avail, display::RESET);
        println!("  {}GPU:{}         {}{}{}", display::DIM, display::RESET, display::WHITE, gpu_name, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
