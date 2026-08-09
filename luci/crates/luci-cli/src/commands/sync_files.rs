use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, source: &str, target: &str) -> Result<()> {
    display::print_header("SYNCING FILES");

    println!("  {}Source:{}    {}{}{}", display::DIM, display::RESET, display::WHITE, source, display::RESET);
    println!("  {}Target:{}    {}{}{}", display::DIM, display::RESET, display::WHITE, target, display::RESET);
    println!();

    let response = client.send(IpcCommand::SyncFiles {
        source_path: source.to_string(),
        target_node: target.to_string(),
    })?;

    if response.is_success() {
        display::print_success("Files synced successfully!");
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

pub async fn list_synced(client: &IpcClient) -> Result<()> {
    display::print_header("SYNCED FOLDERS");

    let response = client.send(IpcCommand::ListSyncedFolders)?;

    if let Some(payload) = response.payload() {
        let empty_vec = vec![];
        let folders = payload["synced_folders"].as_array().unwrap_or(&empty_vec);

        if folders.is_empty() {
            display::print_info("No synced folders.");
            println!();
            return Ok(());
        }

        for folder in folders {
            let source = folder["source_path"].as_str().unwrap_or("unknown");
            let target = folder["target_node"].as_str().unwrap_or("unknown");
            let files = folder["files_synced"].as_u64().unwrap_or(0);

            println!("  {}{}->{} {}  ->  {}{}  ({} files)", 
                display::CYAN, display::BOLD, display::RESET, source, display::WHITE, target, files);
        }
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
