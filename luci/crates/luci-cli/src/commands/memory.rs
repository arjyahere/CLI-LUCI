use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn create(client: &IpcClient, name: &str, size_gb: f64) -> Result<()> {
    display::print_header("CREATING MEMORY POOL");

    let response = client.send(IpcCommand::CreateMemoryPool {
        name: name.to_string(),
        size_gb,
    })?;

    if let Some(payload) = response.payload() {
        let pool_id = payload["id"].as_str().unwrap_or("unknown");

        display::print_success(&format!("Memory pool '{}' created!", name));
        println!();
        println!("  {}Name:{}      {}{}{}", display::DIM, display::RESET, display::MAGENTA, name, display::RESET);
        println!("  {}Pool ID:{}   {}{}{}", display::DIM, display::RESET, display::BRIGHT_CYAN, pool_id, display::RESET);
        println!("  {}Size:{}      {}{:.1} GB{}", display::DIM, display::RESET, display::BRIGHT_GREEN, size_gb, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

pub async fn list(client: &IpcClient) -> Result<()> {
    display::print_header("MEMORY POOLS");

    let response = client.send(IpcCommand::ListMemoryPools)?;

    if let Some(payload) = response.payload() {
        let empty_vec = vec![];
        let pools = payload["pools"].as_array().unwrap_or(&empty_vec);

        if pools.is_empty() {
            display::print_info("No memory pools found.");
            println!("  {}Create one:  {}luci memory create <name> <size_gb>{}", display::DIM, display::CYAN, display::RESET);
            println!();
            return Ok(());
        }

        println!("  {}{}┌──────────────────────────────────────────────────────────────┐{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}│ {:<20} │ {:<10} │ {:<12} │ {:<15} │{}", display::DIM, display::BOLD, "NAME", "SIZE", "PAGES", "ID", display::RESET);
        println!("  {}{}├──────────────────────────────────────────────────────────────┤{}", display::DIM, display::BOLD, display::RESET);

        for pool in pools {
            let name = pool["name"].as_str().unwrap_or("unknown");
            let id = pool["id"].as_str().unwrap_or("unknown");
            let size = pool["total_size_gb"].as_f64().unwrap_or(0.0);
            let pages = pool["total_pages"].as_u64().unwrap_or(0);

            println!("  {}│ {:<20} │ {:<10} │ {:<12} │ {:<15} │{}", 
                display::DIM,
                name,
                format!("{:.1}GB", size),
                pages.to_string(),
                &id[..15.min(id.len())],
                display::RESET);
        }

        println!("  {}{}└──────────────────────────────────────────────────────────────┘{}", display::DIM, display::BOLD, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

pub async fn info(client: &IpcClient, id: &str) -> Result<()> {
    display::print_header("MEMORY POOL INFO");

    let response = client.send(IpcCommand::GetMemoryPoolInfo {
        pool_id: id.to_string(),
    })?;

    if let Some(payload) = response.payload() {
        let name = payload["name"].as_str().unwrap_or("unknown");
        let size = payload["total_size_gb"].as_f64().unwrap_or(0.0);
        let pages = payload["total_pages"].as_u64().unwrap_or(0);

        println!("  {}Name:{}     {}{}{}", display::DIM, display::RESET, display::WHITE, name, display::RESET);
        println!("  {}Size:{}     {}{:.1} GB{}", display::DIM, display::RESET, display::WHITE, size, display::RESET);
        println!("  {}Pages:{}    {}{}{}", display::DIM, display::RESET, display::WHITE, pages, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

pub async fn remove(client: &IpcClient, id: &str) -> Result<()> {
    display::print_header("DELETING MEMORY POOL");

    let response = client.send(IpcCommand::DeleteMemoryPool {
        pool_id: id.to_string(),
    })?;

    if response.is_success() {
        display::print_success("Memory pool deleted.");
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
