use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, name: Option<&str>) -> Result<()> {
    display::print_header("LEAVING NETWORK");

    let response = client.send(IpcCommand::LeaveNetwork {
        name: name.map(|s| s.to_string()),
    })?;

    if response.is_success() {
        display::print_success("Left the network successfully.");
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
