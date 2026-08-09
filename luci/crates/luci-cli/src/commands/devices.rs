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

        println!("  {}{}+==============================================================+{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|                      NETWORK DEVICES                         |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}+==============================================================+{}", display::DIM, display::BOLD, display::RESET);
        println!();

        if count == 0 {
            display::print_warning("No devices found in the network.");
            println!("  {}Get started:{}", display::DIM, display::RESET);
            println!("    {}luci create <name>{}    Create a new network", display::CYAN, display::RESET);
            println!("    {}/join <code>{}     Join an existing network", display::CYAN, display::RESET);
            println!();
            return Ok(());
        }

        println!("  {}{}NETWORK DEVICES ({}){}", display::BOLD, display::CYAN, count, display::RESET);
        println!("  {}{}{}", display::DIM, "-".repeat(72), display::RESET);
        println!();

        for device in devices {
            let name = device["name"].as_str().unwrap_or("unknown");
            let status = device["status"].as_str().unwrap_or("unknown");
            let hostname = device["hostname"].as_str().unwrap_or("");
            let os = device["os"].as_str().unwrap_or("");
            let ip = device["ip"].as_str().unwrap_or("");
            let mac = device["mac_address"].as_str().unwrap_or("");
            let connected_ip = device["connected_ip"].as_str().unwrap_or("");
            let cpu_load = device["cpu_load"].as_f64().unwrap_or(0.0);
            let cpu_cores = device["cpu_cores"].as_u64().unwrap_or(0);
            let cpu_freq = device["cpu_freq_mhz"].as_u64().unwrap_or(0);
            let ram_total = device["ram_total_gb"].as_f64().unwrap_or(0.0);
            let ram_used = device["ram_used_gb"].as_f64().unwrap_or(0.0);
            let ram_pct = device["ram_percent"].as_f64().unwrap_or(0.0);
            let disk_total = device["disk_total_gb"].as_f64().unwrap_or(0.0);
            let disk_free = device["disk_free_gb"].as_f64().unwrap_or(0.0);
            let gpu_name = device["gpu_name"].as_str().unwrap_or("-");
            let gpu_util = device["gpu_util"].as_f64();

            let status_display = display::format_node_status(status);
            println!("  {}{}{}  {}", display::BOLD, display::WHITE, name, display::RESET);
            println!("    Status       {}", status_display);
            if !hostname.is_empty() || !os.is_empty() {
                let mut meta = Vec::new();
                if !hostname.is_empty() { meta.push(hostname.to_string()); }
                if !os.is_empty() { meta.push(os.to_string()); }
                println!("    System       {}", meta.join("  |  "));
            }
            println!("    IP           {}", if ip.is_empty() { "-" } else { ip });
            println!("    Connected to {}", if connected_ip.is_empty() { "-" } else { connected_ip });
            println!("    MAC          {}", if mac.is_empty() { "-" } else { mac });

            let cpu_bar = display::progress_bar(cpu_load, 100.0, 20);
            let cpu_freq_str = if cpu_freq > 0 { format!("  {:.1} GHz", cpu_freq as f64 / 1000.0) } else { String::new() };
            println!("    CPU      {}  {:>5.1}%{}  ({} cores)", cpu_bar, cpu_load, cpu_freq_str, cpu_cores);

            let mem_bar = display::progress_bar(ram_pct, 100.0, 20);
            println!("    Memory   {}  {:.1}/{:.1} GB ({:.0}%)", mem_bar, ram_used, ram_total, ram_pct);

            if disk_total > 0.0 {
                let disk_used = disk_total - disk_free;
                let disk_pct = if disk_total > 0.0 { (disk_used / disk_total * 100.0) as f64 } else { 0.0 };
                let disk_bar = display::progress_bar(disk_pct, 100.0, 20);
                println!("    Disk     {}  {:.1}/{:.1} GB ({:.0}%)", disk_bar, disk_used, disk_total, disk_pct);
            }

            if gpu_name != "-" {
                let gutil = gpu_util.unwrap_or(0.0);
                let gpu_bar = display::progress_bar(gutil, 100.0, 20);
                println!("    GPU      {}  {:>5.1}%  {}", gpu_bar, gutil, gpu_name);
            } else {
                println!("    GPU      {}", "-".to_string());
            }
            println!();
        }
        println!();
        println!("  {}Total: {} device(s) connected{}", display::DIM, count, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
