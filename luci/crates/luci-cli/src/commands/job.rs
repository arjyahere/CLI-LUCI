use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, id: &str) -> Result<()> {
    display::print_header("JOB DETAILS");

    let response = client.send(IpcCommand::GetJobStatus {
        job_id: id.to_string(),
    })?;

    if let Some(payload) = response.payload() {
        let name = payload["name"].as_str().unwrap_or("unknown");
        let job_type = payload["type"].as_str().unwrap_or("unknown");
        let status = payload["status"].as_str().unwrap_or("unknown");
        let node = payload["assigned_node"].as_str().unwrap_or("-");
        let result = payload["result"].as_str();
        let error = payload["error"].as_str();

        let status_display = display::format_job_status(status);

        println!("  {}Name:{}       {}{}{}", display::DIM, display::RESET, display::WHITE, name, display::RESET);
        println!("  {}Type:{}       {}{}{}", display::DIM, display::RESET, display::WHITE, job_type, display::RESET);
        println!("  {}Status:{}     {}", display::DIM, display::RESET, status_display);
        println!("  {}Node:{}       {}{}{}", display::DIM, display::RESET, display::WHITE, node, display::RESET);
        println!("  {}ID:{}         {}{}{}", display::DIM, display::RESET, display::CYAN, id, display::RESET);
        
        if let Some(result) = result {
            println!();
            println!("  {}{}--- Result --------------------------------------------------{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}{}{}{}", display::GREEN, result, display::RESET, display::DIM);
        }
        
        if let Some(error) = error {
            println!();
            println!("  {}{}--- Error ---------------------------------------------------{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}{}{}{}", display::RED, error, display::RESET, display::DIM);
        }
        
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
