use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn start(client: &IpcClient, port: u16, nodes: &[String]) -> Result<()> {
    display::print_header("STARTING PROXY");

    let response = client.send(IpcCommand::StartProxy {
        listen_port: port,
        target_nodes: nodes.to_vec(),
    })?;

    if response.is_success() {
        display::print_success(&format!("Proxy started on port {}", port));
        display::print_kv("Port", &format!("{}{}{}", display::BRIGHT_MAGENTA, port, display::RESET));
        display::print_kv("Target nodes", &format!("{:?}", nodes));
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

pub async fn stop(client: &IpcClient) -> Result<()> {
    display::print_header("STOPPING PROXY");

    let response = client.send(IpcCommand::StopProxy)?;

    if response.is_success() {
        display::print_success("Proxy stopped.");
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

pub async fn status(client: &IpcClient) -> Result<()> {
    display::print_header("PROXY STATUS");

    let response = client.send(IpcCommand::GetProxyStatus)?;

    if let Some(payload) = response.payload() {
        let running = payload["running"].as_bool().unwrap_or(false);
        let port = payload["listen_port"].as_u64().unwrap_or(0);
        let requests = payload["total_requests"].as_u64().unwrap_or(0);
        let errors = payload["total_errors"].as_u64().unwrap_or(0);

        let status_display = display::status_indicator(if running { "online" } else { "offline" });

        println!("  {}Status:{}     {} {}{}", display::DIM, display::RESET, status_display, if running { "Running" } else { "Stopped" }, display::RESET);
        display::print_kv("Port", &format!("{}{}{}", display::BRIGHT_MAGENTA, port, display::RESET));
        display::print_kv("Requests", &requests.to_string());
        display::print_kv_status("Errors", &errors.to_string(), errors == 0);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
