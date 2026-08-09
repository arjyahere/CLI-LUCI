use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, name: &str) -> Result<()> {
    display::print_banner();
    display::print_header("CREATING NETWORK");

    println!("  {}Creating network: {}{}{}{}", display::DIM, display::WHITE, name, display::RESET, display::DIM);
    println!();

    let response = client.send(IpcCommand::CreateNetwork {
        name: name.to_string(),
    })?;

    if let Some(payload) = response.payload() {
        let network_id = payload["network_id"].as_str().unwrap_or("unknown");
        let invite_code = payload["invite_code"].as_str().unwrap_or("unknown");

        display::print_success("Network created successfully!");
        println!();

        println!("  {}{}+--------------------------------------------------------------+{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|                        NETWORK INFO                          |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}+--------------------------------------------------------------+{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|                                                              |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|  {}Network:{}     {:<47}|{}", display::DIM, display::BOLD, display::RESET, display::CYAN, name, display::RESET);
        println!("  {}{}|  {}Network ID:{}  {:<47}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, network_id, display::RESET);
        println!("  {}{}|                                                              |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|  {}Invite Code:{}                                                |{}", display::DIM, display::BOLD, display::RESET, display::DIM, display::RESET);
        println!("  {}{}|  {}{}{:<60}{}|{}", display::DIM, display::BOLD, display::RESET, display::BRIGHT_YELLOW, invite_code, display::DIM, display::RESET);
        println!("  {}{}|                                                              |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}+--------------------------------------------------------------+{}", display::DIM, display::BOLD, display::RESET);
        println!();

        println!("  {}{}Share this invite code with others to join your network:{}", display::DIM, display::BOLD, display::RESET);
        println!();
        println!("    {}/join {}{}{}", display::CYAN, display::BRIGHT_YELLOW, invite_code, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
