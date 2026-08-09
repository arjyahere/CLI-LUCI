use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_banner();
    display::print_header("HEALTH STATUS");

    let response = client.send(IpcCommand::GetHealth)?;

    if let Some(payload) = response.payload() {
        let status = payload["status"].as_str().unwrap_or("unknown");
        let version = payload["version"].as_str().unwrap_or("unknown");
        let uptime = payload["uptime_secs"].as_u64().unwrap_or(0);
        let uptime_str = display::format_duration(uptime);

        let status_display = if status == "healthy" {
            format!("{}{}● Healthy{}", display::BRIGHT_GREEN, display::BOLD, display::RESET)
        } else {
            format!("{}{}● Unhealthy{}", display::BRIGHT_RED, display::BOLD, display::RESET)
        };

        println!("  {}Status:{}     {}", display::DIM, display::RESET, status_display);
        display::print_kv("Version", &format!("{}{}{}", display::BRIGHT_BLUE, version, display::RESET));
        display::print_kv("Uptime", &uptime_str);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
