use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub async fn execute(client: &IpcClient) -> Result<()> {
    display::print_banner();

    let response = client.send(IpcCommand::ListJobs)?;

    if let Some(payload) = response.payload() {
        let empty_vec = vec![];
        let jobs = payload["jobs"].as_array().unwrap_or(&empty_vec);

        println!("  {}{}+==============================================================+{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}|                        ACTIVE JOBS                           |{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}+==============================================================+{}", display::DIM, display::BOLD, display::RESET);
        println!();

        if jobs.is_empty() {
            display::print_info("No jobs running.");
            println!("  {}Run a job:  {}/run <script>{}", display::DIM, display::CYAN, display::RESET);
            println!();
            return Ok(());
        }

        // Table header
        println!("  {}{}+--------------+--------------------+----------+------------+--------------+{}", display::DIM, display::BOLD, display::RESET);
        println!("  {}{}| {:<12} | {:<18} | {:<8} | {:<10} | {:<12} |{}", display::DIM, display::BOLD, 
            "ID", "NAME", "TYPE", "STATUS", "NODE", display::RESET);
        println!("  {}{}+--------------+--------------------+----------+------------+--------------+{}", display::DIM, display::BOLD, display::RESET);

        for job in jobs {
            let id = job["id"].as_str().unwrap_or("unknown");
            let name = job["name"].as_str().unwrap_or("unknown");
            let job_type = job["type"].as_str().unwrap_or("unknown");
            let status = job["status"].as_str().unwrap_or("unknown");
            let node = job["assigned_node"].as_str().unwrap_or("-");

            let id_display = if id.len() > 12 { &id[..12] } else { id };
            let name_display = if name.len() > 18 { &name[..18] } else { name };
            let node_display = if node.len() > 12 { &node[..12] } else { node };

            let status_display = display::format_job_status(status);

            println!("  {}| {:<12} | {:<18} | {:<8} | {}{:<10}{} | {:<12} |{}", 
                display::DIM,
                id_display,
                name_display,
                job_type,
                display::RESET, status_display, display::RESET,
                node_display,
                display::RESET);
        }

        println!("  {}{}+--------------+--------------------+----------+------------+--------------+{}", display::DIM, display::BOLD, display::RESET);
        println!();
        println!("  {}Total: {} job(s){}", display::DIM, jobs.len(), display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}
