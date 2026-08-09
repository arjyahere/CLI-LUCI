use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient, id: &str) -> Result<()> {
    display::print_header("CANCELLING JOB");

    let response = client.send(IpcCommand::CancelJob {
        job_id: id.to_string(),
    })?;

    if response.is_success() {
        display::print_success(&format!("Job {} cancelled.", id));
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
