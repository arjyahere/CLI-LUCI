pub mod daemon;
pub mod ipc_server;

use anyhow::Result;
use luci_common::Config;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

fn pause_if_own_console() {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetConsoleProcessList(process_list: *mut u32, count: u32) -> u32;
        }
        let mut list = [0u32; 2];
        let count = unsafe { GetConsoleProcessList(list.as_mut_ptr(), 2) };
        if count == 1 {
            eprintln!("\nPress Enter to exit...");
            let mut input = String::new();
            let _ = std::io::stdin().read_line(&mut input);
        }
    }
}

#[tokio::main]
async fn main() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        pause_if_own_console();
    }));
    let code = match run().await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e:#}");
            1
        }
    };
    pause_if_own_console();
    std::process::exit(code);
}

async fn run() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let config = Config::load()?;
    config.ensure_dirs()?;

    let file_appender = tracing_appender::rolling::never(config.data_dir(), "lucid.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_filter(env_filter.clone());

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    tracing::info!("Starting lucid daemon...");
    tracing::info!("Node ID: {}", config.node_id);
    tracing::info!("Port: {}", config.daemon_port);

    // Auto-configure firewall on Windows
    #[cfg(target_os = "windows")]
    setup_firewall(&config);

    let daemon = daemon::Daemon::new(config).await?;
    
    tracing::info!("Daemon started (PID: {})", std::process::id());

    // Run daemon
    daemon.run().await?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn setup_firewall(config: &Config) {
    use std::process::Command;

    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_str = exe_path.to_string_lossy();
    let node_port = config.node_port.to_string();
    let discovery_port = config.discovery_port.to_string();

    let rules = [
        ("Luci Daemon Inbound", "inbound", &node_port, "TCP"),
        ("Luci Discovery Inbound", "inbound", &discovery_port, "UDP"),
        ("Luci Daemon Outbound", "outbound", &node_port, "TCP"),
        ("Luci Discovery Outbound", "outbound", &discovery_port, "UDP"),
    ];

    for (name, dir, port, proto) in &rules {
        match Command::new("netsh")
            .args([
                "advfirewall", "firewall", "add", "rule",
                "name=", name,
                "dir=", dir,
                "action=", "allow",
                "program=", &exe_str,
                "protocol=", proto,
                if *dir == "inbound" { "localport" } else { "remoteport" }, port,
                "profile=", "private",
            ])
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("Failed to add firewall rule '{}': {}", name, stderr.trim());
                }
            }
            Err(e) => {
                tracing::warn!("Failed to run netsh for rule '{}': {}", name, e);
            }
        }
    }

    tracing::info!("Firewall rules configured");
}
