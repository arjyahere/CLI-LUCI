use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, format: &str) -> Result<()> {
    display::print_header("METRICS");

    let response = client.send(IpcCommand::GetMetrics)?;

    if let Some(payload) = response.payload() {
        if format == "json" {
            println!("{}", serde_json::to_string_pretty(payload).unwrap_or_default());
        } else if format == "prometheus" {
            // Prometheus format
            let jobs_submitted = payload["jobs_submitted"].as_u64().unwrap_or(0);
            let jobs_completed = payload["jobs_completed"].as_u64().unwrap_or(0);
            let jobs_failed = payload["jobs_failed"].as_u64().unwrap_or(0);
            
            println!("# HELP luci_jobs_submitted Total jobs submitted");
            println!("# TYPE luci_jobs_submitted counter");
            println!("luci_jobs_submitted {}", jobs_submitted);
            println!("# HELP luci_jobs_completed Total jobs completed");
            println!("# TYPE luci_jobs_completed counter");
            println!("luci_jobs_completed {}", jobs_completed);
            println!("# HELP luci_jobs_failed Total jobs failed");
            println!("# TYPE luci_jobs_failed counter");
            println!("luci_jobs_failed {}", jobs_failed);
        } else {
            // Text format
            let jobs_submitted = payload["jobs_submitted"].as_u64().unwrap_or(0);
            let jobs_completed = payload["jobs_completed"].as_u64().unwrap_or(0);
            let jobs_failed = payload["jobs_failed"].as_u64().unwrap_or(0);
            let bytes_transferred = payload["bytes_transferred"].as_u64().unwrap_or(0);
            let connections = payload["active_connections"].as_u64().unwrap_or(0);

            println!("  {}{}--- Job Metrics --------------------------------------------{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}Submitted:{}    {}{} jobs{}", display::DIM, display::RESET, display::WHITE, jobs_submitted, display::RESET);
            println!("  {}Completed:{}    {}{} jobs{}", display::DIM, display::RESET, display::GREEN, jobs_completed, display::RESET);
            println!("  {}Failed:{}       {}{} jobs{}", display::DIM, display::RESET, display::RED, jobs_failed, display::RESET);
            println!();
            println!("  {}{}--- Network Metrics ---------------------------------------{}", display::DIM, display::BOLD, display::RESET);
            println!("  {}Bytes Sent:{}    {}{}{}", display::DIM, display::RESET, display::WHITE, display::format_bytes(bytes_transferred), display::RESET);
            println!("  {}Connections:{}  {}{}{}", display::DIM, display::RESET, display::WHITE, connections, display::RESET);
            println!();
        }
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
