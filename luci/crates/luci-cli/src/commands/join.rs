use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, invite_code: &str) -> Result<()> {
    display::print_banner();
    display::print_header("JOINING NETWORK");

    println!("  {}Joining network with invite code...{}", display::DIM, display::RESET);
    println!();

    let response = client.send(IpcCommand::JoinNetwork {
        invite_code: invite_code.to_string(),
    })?;

    if response.is_success() {
        display::print_success("Successfully joined the network!");
        println!();
        println!("  {}You are now connected to the distributed cluster.{}", display::DIM, display::RESET);
        println!();
        println!("  {}Try these commands:{}", display::DIM, display::RESET);
        println!("    {}luci devices{}       View all nodes in the network", display::CYAN, display::RESET);
        println!("    {}luci resources{}    See combined cluster resources", display::CYAN, display::RESET);
        println!("    {}luci run <script>{}  Execute a job on the cluster", display::CYAN, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
