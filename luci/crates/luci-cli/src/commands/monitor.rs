use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;
use std::time::Duration;

pub async fn execute(client: &IpcClient, interval: u64, node: Option<&str>) -> Result<()> {
    display::print_banner();

    if let Some(node_name) = node {
        println!("  {}{}Monitoring Node: {}{}", display::BOLD, display::BRIGHT_CYAN, node_name, display::RESET);
    } else {
        println!("  {}{}Real-time Cluster Monitor{}", display::BOLD, display::BRIGHT_CYAN, display::RESET);
    }
    println!("  {}Press Ctrl+C to exit{}", display::DIM, display::RESET);
    println!();

    loop {
        // Clear screen and move cursor to top
        print!("\x1B[2J\x1B[1;1H");
        
        display::print_banner();

        if let Some(node_name) = node {
            // Single node view
            println!("  {}{}┌──────────────────────────────────────────────────────────────┐{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}{}│                  NODE MONITOR: {:<32}│{}", display::DIM, display::BOLD, node_name, display::RESET);
            println!("  {}{}└──────────────────────────────────────────────────────────────┘{}", display::DIM, display::BOLD, display::RESET);
            println!();

            let response = client.send(IpcCommand::GetDevice { name: node_name.to_string() })?;
            
            if let Some(payload) = response.payload() {
                let status = payload["status"].as_str().unwrap_or("unknown");
                let cpu_cores = payload["cpu_cores"].as_u64().unwrap_or(0);
                let cpu_load = payload["cpu_load"].as_f64().unwrap_or(0.0);
                let ram_total = payload["ram_total_gb"].as_f64().unwrap_or(0.0);
                let ram_avail = payload["ram_available_gb"].as_f64().unwrap_or(0.0);
                let gpu_name = payload["gpu_name"].as_str().unwrap_or("-");

                let status_display = display::format_node_status(status);
                let ram_used = ram_total - ram_avail;
                let ram_pct = if ram_total > 0.0 { (ram_used / ram_total) * 100.0 } else { 0.0 };

                println!("  {}Status:{}     {}", display::DIM, display::RESET, status_display);
                println!("  {}CPU:{}         {}{} cores (load: {:.1}%){}", display::DIM, display::RESET, display::WHITE, cpu_cores, cpu_load, display::RESET);
                
                let ram_bar = display::progress_bar(ram_used, ram_total, 40);
                println!("  {}Memory:{}      [{}] {:.1}GB / {:.1}GB ({:.1}%)", 
                    display::DIM, display::RESET, ram_bar, ram_used, ram_total, ram_pct);
                
                println!("  {}GPU:{}         {}{}{}", display::DIM, display::RESET, display::WHITE, gpu_name, display::RESET);
            } else if let Some(msg) = response.error_message() {
                display::print_error(msg);
            }
        } else {
            // Cluster view
            println!("  {}{}┌──────────────────────────────────────────────────────────────┐{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}{}│                  CLUSTER MONITOR                             │{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}{}└──────────────────────────────────────────────────────────────┘{}", display::DIM, display::BOLD, display::RESET);
            println!();

            let response = client.send(IpcCommand::GetResources)?;
            
            if let Some(payload) = response.payload() {
                let devices = payload["devices"].as_u64().unwrap_or(0);
                let cpu_cores = payload["cpu_cores"].as_u64().unwrap_or(0);
                let ram_total = payload["ram_total_gb"].as_str().unwrap_or("0.0").parse::<f64>().unwrap_or(0.0);
                let ram_avail = payload["ram_available_gb"].as_str().unwrap_or("0.0").parse::<f64>().unwrap_or(0.0);
                let gpu_count = payload["gpu_count"].as_u64().unwrap_or(0);

                let ram_used = ram_total - ram_avail;
                let ram_pct = if ram_total > 0.0 { (ram_used / ram_total) * 100.0 } else { 0.0 };

                println!("  {}Nodes:{}        {}{} connected{}", display::DIM, display::RESET, display::GREEN, devices, display::RESET);
                println!("  {}CPU Cores:{}    {}{} cores{}", display::DIM, display::RESET, display::BRIGHT_CYAN, cpu_cores, display::RESET);
                println!();
                
                let ram_bar = display::progress_bar(ram_used, ram_total, 40);
                println!("  {}Memory:{}       [{}] {:.1}GB / {:.1}GB ({:.1}%)", 
                    display::DIM, display::RESET, ram_bar, ram_used, ram_total, ram_pct);
                
                if gpu_count > 0 {
                    println!("  {}GPUs:{}         {}{} device(s){}", display::DIM, display::RESET, display::GREEN, gpu_count, display::RESET);
                }
                
                println!();
                println!("  {}Last updated: {}{}", display::DIM, 
                    chrono::Utc::now().format("%H:%M:%S"), display::RESET);
            }
        }

        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}
