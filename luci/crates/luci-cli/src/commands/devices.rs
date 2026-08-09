use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_banner();

    let response = client.send(IpcCommand::ListDevices)?;

    if let Some(payload) = response.payload() {
        let empty_vec = vec![];
        let devices = payload["devices"].as_array().unwrap_or(&empty_vec);
        let count = payload["count"].as_u64().unwrap_or(0);

        println!("  {}{}╔══════════════════════════════════════════════════════════════╗{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}║                      NETWORK DEVICES                         ║{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}╚══════════════════════════════════════════════════════════════╝{}", display::DIM, display::BOLD, display::RESET);
        println!();

        if count == 0 {
            display::print_warning("No devices found in the network.");
            println!("  {}Get started:{}", display::DIM, display::RESET);
            println!("    {}luci create <name>{}    Create a new network", display::CYAN, display::RESET);
            println!("    {}/join <code>{}     Join an existing network", display::CYAN, display::RESET);
            println!();
            return Ok(());
        }

        // Table header
        println!("  {}{}┌──────────────────┬────────────┬──────────┬────────────┬────────────┬──────────────┐{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}│ {:<16} │ {:<10} │ {:<8} │ {:<10} │ {:<10} │ {:<12} │{}", display::DIM, display::BOLD, 
            "NAME", "STATUS", "CPU", "RAM", "FREE", "GPU", display::RESET);
        println!("  {}{}├──────────────────┼────────────┼──────────┼────────────┼────────────┼──────────────┤{}", display::DIM, display::BOLD, display::RESET);

        for device in devices {
            let name = device["name"].as_str().unwrap_or("unknown");
            let status = device["status"].as_str().unwrap_or("unknown");
            let cpu_cores = device["cpu_cores"].as_u64().unwrap_or(0);
            let cpu_load = device["cpu_load"].as_f64().unwrap_or(0.0);
            let ram_total = device["ram_total_gb"].as_f64().unwrap_or(0.0);
            let ram_avail = device["ram_available_gb"].as_f64().unwrap_or(0.0);
            let gpu_name = device["gpu_name"].as_str().unwrap_or("-");

            let status_display = display::format_node_status(status);
            let cpu_display = format!("{}c/{:.0}%", cpu_cores, cpu_load);
            let ram_display = format!("{:.1}GB", ram_total);
            let avail_display = format!("{:.1}GB", ram_avail);
            let gpu_display = if gpu_name == "-" { "-".to_string() } else { gpu_name.chars().take(12).collect() };

            println!("  {}│ {:<16} │ {}{:<10}{} │ {:<8} │ {:<10} │ {:<10} │ {:<12} │{}", 
                display::DIM,
                name.chars().take(16).collect::<String>(),
                display::RESET, status_display, display::DIM,
                cpu_display,
                ram_display,
                avail_display,
                gpu_display,
                display::RESET);
        }

        println!("  {}{}└──────────────────┴────────────┴──────────┴────────────┴────────────┴──────────────┘{}", display::DIM, display::BOLD, display::RESET);
        println!();
        println!("  {}Total: {} device(s) connected{}", display::DIM, count, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
