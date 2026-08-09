mod commands;
mod display;
mod ipc_client;

use clap::{Parser, Subcommand};
use ipc_client::IpcClient;
use luci_common::Config;

#[derive(Parser)]
#[command(
    name = "luci",
    about = "One Runtime. Infinite Resources.",
    version,
    long_about = "Luci is a distributed runtime that virtualizes compute, memory, and AI acceleration across trusted devices.",
    after_help = "Examples:\n  luci create home          Create a new network\n  luci join AB4K-X91M-QW34   Join an existing network\n  luci devices               List all devices\n  luci resources             Show combined resources\n  luci run payroll.py        Run a script"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Daemon port (default: 9527)
    #[arg(long, global = true)]
    port: Option<u16>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new network
    Create {
        /// Network name
        name: String,
    },

    /// Join an existing network
    Join {
        /// Invite code from the network creator
        invite_code: String,
    },

    /// Leave the current network
    Leave {
        /// Network name to leave
        name: Option<String>,
    },

    /// List all devices in the network
    Devices,

    /// Show device details
    Device {
        /// Device name or ID
        name: String,
    },

    /// Show combined resources
    Resources,

    /// Run a script or command
    Run {
        /// Script or command to run
        script: String,

        /// Arguments to pass
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,

        /// Required RAM (e.g., 4GB, 8GB)
        #[arg(long)]
        ram: Option<String>,

        /// Required CPU cores
        #[arg(long)]
        cpu: Option<usize>,

        /// Run on specific node
        #[arg(long)]
        node: Option<String>,

        /// Require GPU
        #[arg(long)]
        gpu: bool,

        /// Minimum VRAM (e.g., 4GB, 8GB)
        #[arg(long)]
        vram: Option<String>,

        /// Max job duration in seconds
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// List all jobs
    Jobs,

    /// Show job status
    Job {
        /// Job ID
        id: String,
    },

    /// Cancel a job
    Cancel {
        /// Job ID
        id: String,
    },

    /// Memory management
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },

    /// Show daemon status
    Status,

    /// Start the daemon
    Start,

    /// Stop the daemon
    Stop,

    /// Restart the daemon
    Restart,

    /// View logs
    Logs {
        /// Filter by level (info, warn, error)
        #[arg(long)]
        filter: Option<String>,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },

    /// Real-time resource monitoring across all nodes
    Monitor {
        /// Refresh interval in seconds (default: 2)
        #[arg(short, long, default_value = "2")]
        interval: u64,

        /// Monitor specific node only
        #[arg(long)]
        node: Option<String>,
    },

    /// Sync project folder to peer nodes
    Sync {
        /// Source folder path
        source: String,

        /// Target node name or ID (or "all" for all nodes)
        target: String,
    },

    /// List synced folders
    Synced,

    /// Start reverse proxy for long-running servers
    Proxy {
        #[command(subcommand)]
        command: ProxyCommands,
    },

    /// Show auto-failover status
    Failover,

    /// List discovered peers
    Peers,

    /// Show health status
    Health,

    /// Show metrics
    Metrics {
        /// Output format (text, json, prometheus)
        #[arg(long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// Create a virtual memory pool
    Create {
        /// Pool name
        name: String,

        /// Size in GB (e.g., 8, 16, 32)
        size_gb: f64,
    },

    /// List memory pools
    Ls,

    /// Show memory pool details
    Info {
        /// Memory pool ID
        id: String,
    },

    /// Delete a memory pool
    Rm {
        /// Memory pool ID
        id: String,
    },
}

#[derive(Subcommand)]
enum ProxyCommands {
    /// Start reverse proxy
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Target node(s) to proxy to
        #[arg(long)]
        nodes: Vec<String>,
    },

    /// Stop reverse proxy
    Stop,

    /// Show proxy status
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let port = cli.port.unwrap_or_else(|| {
        Config::load()
            .map(|c| c.daemon_port)
            .unwrap_or(9527)
    });

    let client = IpcClient::new(port);

    match cli.command {
        Commands::Create { name } => commands::create::execute(&client, &name).await,
        Commands::Join { invite_code } => commands::join::execute(&client, &invite_code).await,
        Commands::Leave { name } => commands::leave::execute(&client, name.as_deref()).await,
        Commands::Devices => commands::devices::execute(&client).await,
        Commands::Device { name } => commands::device::execute(&client, &name).await,
        Commands::Resources => commands::resources::execute(&client).await,
        Commands::Run {
            script,
            args,
            ram,
            cpu,
            node,
            gpu,
            vram,
            timeout,
        } => {
            commands::run::execute(
                &client,
                commands::run::RunConfig {
                    script,
                    args,
                    ram,
                    cpu,
                    node,
                    gpu,
                    vram,
                    timeout,
                },
            )
            .await
        }
        Commands::Jobs => commands::jobs::execute(&client).await,
        Commands::Job { id } => commands::job::execute(&client, &id).await,
        Commands::Cancel { id } => commands::cancel::execute(&client, &id).await,
        Commands::Memory { command } => match command {
            MemoryCommands::Create { name, size_gb } => {
                commands::memory::create(&client, &name, size_gb).await
            }
            MemoryCommands::Ls => commands::memory::list(&client).await,
            MemoryCommands::Info { id } => commands::memory::info(&client, &id).await,
            MemoryCommands::Rm { id } => commands::memory::remove(&client, &id).await,
        },
        Commands::Status => commands::status::execute(&client).await,
        Commands::Start => commands::daemon::start(&client).await,
        Commands::Stop => commands::daemon::stop(&client).await,
        Commands::Restart => commands::daemon::restart(&client).await,
        Commands::Logs { filter, follow } => commands::logs::execute(&client, filter.as_deref(), follow).await,
        Commands::Monitor { interval, node } => commands::monitor::execute(&client, interval, node.as_deref()).await,
        Commands::Sync { source, target } => commands::sync_files::execute(&client, &source, &target).await,
        Commands::Synced => commands::sync_files::list_synced(&client).await,
        Commands::Proxy { command } => match command {
            ProxyCommands::Start { port, nodes } => commands::proxy::start(&client, port, &nodes).await,
            ProxyCommands::Stop => commands::proxy::stop(&client).await,
            ProxyCommands::Status => commands::proxy::status(&client).await,
        },
        Commands::Failover => commands::failover::execute(&client).await,
        Commands::Peers => commands::peers::execute(&client).await,
        Commands::Health => commands::health::execute(&client).await,
        Commands::Metrics { format } => commands::metrics::execute(&client, &format).await,
    }
}
