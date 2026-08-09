use anyhow::Result;
use std::process::Command;
use crate::ipc_client::IpcClient;
use crate::display;

pub async fn start(client: &IpcClient) -> Result<()> {
    display::print_header("STARTING DAEMON");

    if client.is_daemon_running() {
        display::print_warning("Daemon is already running.");
        println!();
        return Ok(());
    }

    println!("  {}Starting lucid daemon...{}", display::DIM, display::RESET);

    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "lucid"])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        Command::new("lucid")
            .spawn()?;
    }

    // Wait for daemon to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    if client.is_daemon_running() {
        display::print_success("Daemon started successfully.");
    } else {
        display::print_warning("Daemon started but not yet responding.");
    }

    println!();
    Ok(())
}

pub async fn stop(client: &IpcClient) -> Result<()> {
    display::print_header("STOPPING DAEMON");

    if !client.is_daemon_running() {
        display::print_warning("Daemon is not running.");
        println!();
        return Ok(());
    }

    println!("  {}Stopping lucid daemon...{}", display::DIM, display::RESET);

    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "taskkill", "/IM", "lucid.exe", "/F"])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        Command::new("pkill")
            .arg("lucid")
            .spawn()?;
    }

    // Wait for daemon to stop
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    if !client.is_daemon_running() {
        display::print_success("Daemon stopped successfully.");
    } else {
        display::print_warning("Daemon may still be running.");
    }

    println!();
    Ok(())
}

pub async fn restart(client: &IpcClient) -> Result<()> {
    display::print_header("RESTARTING DAEMON");

    if !client.is_daemon_running() {
        display::print_warning("Daemon is not running. Starting...");
        println!();
        return start(client).await;
    }

    println!("  {}Stopping daemon...{}", display::DIM, display::RESET);

    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "taskkill", "/IM", "lucid.exe", "/F"])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        Command::new("pkill")
            .arg("lucid")
            .spawn()?;
    }

    // Wait for daemon to stop
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    println!("  {}Starting daemon...{}", display::DIM, display::RESET);

    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "lucid"])
            .spawn()?;
    }

    #[cfg(not(windows))]
    {
        Command::new("lucid")
            .spawn()?;
    }

    // Wait for daemon to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    if client.is_daemon_running() {
        display::print_success("Daemon restarted successfully.");
    } else {
        display::print_warning("Daemon restarted but not yet responding.");
    }

    println!();
    Ok(())
}
