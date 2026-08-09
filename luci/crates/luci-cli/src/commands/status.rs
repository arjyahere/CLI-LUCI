use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_banner();

    let response = client.send(IpcCommand::GetDaemonStatus)?;

    if let Some(payload) = response.payload() {
        let running = payload["running"].as_bool().unwrap_or(false);
        let pid = payload["pid"].as_u64().map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
        let uptime = payload["uptime_secs"].as_u64().unwrap_or(0);
        let node_id = payload["node_id"].as_str().unwrap_or("unknown");
        let network_id = payload["network_id"].as_str().unwrap_or("Not connected");
        let connected_nodes = payload["connected_nodes"].as_u64().unwrap_or(0);
        let active_jobs = payload["active_jobs"].as_u64().unwrap_or(0);
        let total_completed = payload["total_jobs_completed"].as_u64().unwrap_or(0);

        println!("  {}{}+==============================================================+{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|                      DAEMON STATUS                           |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}+==============================================================+{}", display::DIM, display::BOLD, display::RESET);
        println!();

        let status_color = if running { display::GREEN } else { display::RED };
        let status_icon = if running { "*" } else { "o" };
        let status_text = if running { "Running" } else { "Stopped" };

        println!("  {}{}{} {}Daemon:{}      {}{}{}", 
            display::BOLD, status_icon, display::RESET,
            display::DIM, display::RESET,
            status_color, status_text, display::RESET);

        println!("  {}PID:{}         {}{}{}", display::DIM, display::RESET, display::WHITE, pid, display::RESET);
        println!("  {}Uptime:{}      {}{}{}", display::DIM, display::RESET, display::WHITE, display::format_duration(uptime), display::RESET);
        println!("  {}Node ID:{}     {}{}{}", display::DIM, display::RESET, display::CYAN, &node_id[..12.min(node_id.len())], display::RESET);
        println!();

        println!("  {}{}--- Network -----------------------------------------------{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}Network:{}     {}{}{}", display::DIM, display::RESET, display::WHITE, network_id, display::RESET);
        println!("  {}Connected:{}   {}{} node(s){}", display::DIM, display::RESET, display::GREEN, connected_nodes, display::RESET);
        println!();

        println!("  {}{}--- Jobs --------------------------------------------------{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}Active:{}       {}{} jobs{}", display::DIM, display::RESET, display::YELLOW, active_jobs, display::RESET);
        println!("  {}Completed:{}    {}{} jobs{}", display::DIM, display::RESET, display::GREEN, total_completed, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
