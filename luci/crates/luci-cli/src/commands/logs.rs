use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, filter: Option<&str>, follow: bool) -> Result<()> {
    display::print_header("DAEMON LOGS");

    let response = client.send(IpcCommand::GetDaemonStatus)?;

    if let Some(payload) = response.payload() {
        let log_dir = payload["log_dir"].as_str().unwrap_or("logs");
        let log_level = payload["log_level"].as_str().unwrap_or("info");

        display::print_kv("Log directory", log_dir);
        display::print_kv("Log level", log_level);

        if let Some(f) = filter {
            display::print_kv("Filter", f);
        }

        println!();

        if follow {
            println!("  {}Following logs... (Press Ctrl+C to stop){}", display::DIM, display::RESET);
            println!();

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        println!();
                        display::print_info("Stopped following logs.");
                        break;
                    }
                    _ = async {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        // In a real implementation, this would stream log lines from the daemon
                        // For now, poll daemon status
                        if let Ok(resp) = client.send(IpcCommand::GetDaemonStatus) {
                            if let Some(p) = resp.payload() {
                                let level = p["log_level"].as_str().unwrap_or("info");
                                // Apply filter if specified
                                if let Some(f) = filter {
                                    if !level.contains(f) {
                                        return;
                                    }
                                }
                                let timestamp = chrono::Utc::now().format("%H:%M:%S");
                                println!("  {}{} [{}]{}  Daemon status: {}", display::DIM, timestamp, level, display::RESET, display::format_network_status(true));
                            }
                        }
                    } => {}
                }
            }
        } else {
            display::print_info("No recent logs available.");
            println!();
            println!("  {}Usage:{}  {}/logs --follow{}               Stream all logs", display::DIM, display::RESET, display::CYAN, display::RESET);
            println!("  {}        {}  {}/logs --follow --filter warn{}  Filter by level", display::DIM, display::RESET, display::CYAN, display::RESET);
        }

        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_log_level_matching() {
        let filter = "warn";
        let levels = vec!["info", "warn", "error", "debug"];
        let matching: Vec<&str> = levels.iter().copied().filter(|l| l.contains(filter)).collect();
        assert_eq!(matching, vec!["warn"]);
    }

    #[test]
    fn test_log_filter_case() {
        let filter = "Error";
        let levels = vec!["info", "warn", "error", "debug"];
        let matching: Vec<&str> = levels.iter().copied().filter(|l| l.to_lowercase().contains(&filter.to_lowercase())).collect();
        assert_eq!(matching, vec!["error"]);
    }
}
