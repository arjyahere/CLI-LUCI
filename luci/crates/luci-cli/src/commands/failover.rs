use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_header("FAILOVER STATUS");

    let response = client.send(IpcCommand::GetFailoverStatus)?;

    if let Some(payload) = response.payload() {
        let enabled = payload["enabled"].as_bool().unwrap_or(false);
        let failed_jobs = payload["failed_jobs"].as_u64().unwrap_or(0);
        let restarted_jobs = payload["restarted_jobs"].as_u64().unwrap_or(0);
        let dead_nodes = payload["dead_nodes"].as_u64().unwrap_or(0);

        let status_color = if enabled { display::GREEN } else { display::YELLOW };
        let status_text = if enabled { "Enabled" } else { "Disabled" };

        println!("  {}Status:{}     {}{}{}", display::DIM, display::RESET, status_color, status_text, display::RESET);
        println!("  {}Failed Jobs:{}    {}{}{}", display::DIM, display::RESET, display::RED, failed_jobs, display::RESET);
        println!("  {}Restarted:{}      {}{}{}", display::DIM, display::RESET, display::GREEN, restarted_jobs, display::RESET);
        println!("  {}Dead Nodes:{}     {}{}{}", display::DIM, display::RESET, display::RED, dead_nodes, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
