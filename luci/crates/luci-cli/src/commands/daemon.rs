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

    println!("  {}Starting daemon...{}", display::DIM, display::RESET);
    spawn_self_daemon()?;

    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if client.is_daemon_running() {
            display::print_success("Daemon started successfully.");
            println!();
            return Ok(());
        }
    }

    display::print_warning("Daemon started but not yet responding.");
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

    println!("  {}Stopping daemon...{}", display::DIM, display::RESET);
    let _ = client.send(luci_common::IpcCommand::Shutdown);

    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if !client.is_daemon_running() {
            display::print_success("Daemon stopped successfully.");
            println!();
            return Ok(());
        }
    }

    display::print_warning("Daemon may still be running.");
    println!();
    Ok(())
}

pub async fn restart(client: &IpcClient) -> Result<()> {
    display::print_header("RESTARTING DAEMON");

    if client.is_daemon_running() {
        println!("  {}Stopping daemon...{}", display::DIM, display::RESET);
        let _ = client.send(luci_common::IpcCommand::Shutdown);
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if !client.is_daemon_running() { break; }
        }
    }

    println!("  {}Starting daemon...{}", display::DIM, display::RESET);
    spawn_self_daemon()?;

    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if client.is_daemon_running() {
            display::print_success("Daemon restarted successfully.");
            println!();
            return Ok(());
        }
    }

    display::print_warning("Daemon restarted but not yet responding.");
    println!();
    Ok(())
}

fn spawn_self_daemon() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.arg("--internal-daemon").arg("--hidden");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn()?;
    Ok(())
}
