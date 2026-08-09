use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_header("DISCOVERED PEERS");

    let response = client.send(IpcCommand::GetPeers)?;

    if let Some(payload) = response.payload() {
        let empty_vec = vec![];
        let peers = payload["peers"].as_array().unwrap_or(&empty_vec);

        if peers.is_empty() {
            display::print_info("No peers discovered yet.");
            println!();
            return Ok(());
        }

        for peer in peers {
            let node_id = peer["node_id"].as_str().unwrap_or("unknown");
            let name = peer["node_name"].as_str().unwrap_or("unknown");
            let addr = peer["relay_addresses"].as_array()
                .and_then(|a| a.first())
                .and_then(|a| a.as_str())
                .unwrap_or("unknown");

            println!("  {}{}* {}{}  ({})", display::GREEN, display::BOLD, display::RESET, name, node_id);
            println!("    {}Address: {}{}", display::DIM, display::WHITE, addr);
        }
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
