mod commands;
mod display;
mod ipc_client;

use clap::{CommandFactory, Parser, Subcommand};
use std::io::IsTerminal;
use ipc_client::IpcClient;
use luci_common::Config;

#[derive(Parser)]
#[command(
    name = "luci",
    about = "One Runtime. Infinite Resources.",
    version,
    long_about = "Luci is a distributed runtime that virtualizes compute, memory, and AI acceleration across trusted devices.",
    after_help = "Examples:\n  /create home          Create a new network\n  /join AB4K-X91M-QW34   Join an existing network\n  /devices               List all devices\n  /resources             Show combined resources\n  /run payroll.py        Run a script"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Daemon port (default: 9527)
    #[arg(long, global = true)]
    port: Option<u16>,

    /// Run as the background daemon process (internal)
    #[arg(long, hide = true)]
    internal_daemon: bool,

    /// Hide console window when running internal daemon
    #[arg(long, hide = true)]
    hidden: bool,

    /// Run firewall setup (internal, may be elevated)
    #[arg(long, hide = true)]
    setup_firewall: bool,
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
    Devices {
        /// Refresh continuously
        #[arg(short, long)]
        watch: bool,
    },

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

    /// Daemon window/control
    Daemon {
        #[command(subcommand)]
        action: DaemonControl,
    },

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

#[derive(Subcommand)]
enum DaemonControl {
    /// Show the daemon console window
    Show,
    /// Hide the daemon console window
    Hide,
}

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
    let is_internal = std::env::args().any(|a| a == "--internal-daemon");
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
            fn SetConsoleCP(wCodePageID: u32) -> i32;
        }
        unsafe { SetConsoleOutputCP(65001); SetConsoleCP(65001); }
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        if !is_internal {
            pause_if_own_console();
        }
    }));
    let code = match run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            1
        }
    };
    if !is_internal {
        pause_if_own_console();
    }
    std::process::exit(code);
}

async fn run() -> anyhow::Result<i32> {
    rustls::crypto::ring::default_provider().install_default().ok();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            e.print().ok();
            return Ok(e.exit_code());
        }
    };

    let port = cli.port.unwrap_or_else(|| {
        Config::load()
            .map(|c| c.daemon_port)
            .unwrap_or(9527)
    });

    if cli.setup_firewall {
        let config = Config::load()?;
        #[cfg(windows)]
        {
            let ok = setup_firewall_rules(&config);
            std::process::exit(if ok { 0 } else { 1 });
        }
        #[cfg(not(windows))]
        std::process::exit(0);
    }

    if cli.internal_daemon {
        return run_internal_daemon(port, cli.hidden).await.map(|_| 0);
    }

    let client = IpcClient::new(port);

    match cli.command {
        Some(cmd) => dispatch_command(cmd, &client, port).await,
        None => {
            if std::io::stdin().is_terminal() {
                #[cfg(windows)]
                {
                    unsafe extern "system" {
                        fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
                    }
                    unsafe extern "system" fn ctrl_handler(_: u32) -> i32 { 1 }
                    unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), 1); }
                }
                display::print_banner();
                first_run_setup();
                ensure_daemon_running(&client, port).await;
                run_repl(port).await
            } else {
                Cli::command().print_help().ok();
                println!();
                Ok(0)
            }
        }
    }
}

async fn dispatch_command(cmd: Commands, client: &IpcClient, _port: u16) -> anyhow::Result<i32> {
    let result: anyhow::Result<()> = match cmd {
        Commands::Create { name } => commands::create::execute(client, &name).await,
        Commands::Join { invite_code } => commands::join::execute(client, &invite_code).await,
        Commands::Leave { name } => commands::leave::execute(client, name.as_deref()).await,
        Commands::Devices { watch } => commands::devices::execute(client, watch).await,
        Commands::Device { name } => commands::device::execute(client, &name).await,
        Commands::Resources => commands::resources::execute(client).await,
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
                client,
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
        Commands::Jobs => commands::jobs::execute(client).await,
        Commands::Job { id } => commands::job::execute(client, &id).await,
        Commands::Cancel { id } => commands::cancel::execute(client, &id).await,
        Commands::Memory { command } => match command {
            MemoryCommands::Create { name, size_gb } => {
                commands::memory::create(client, &name, size_gb).await
            }
            MemoryCommands::Ls => commands::memory::list(client).await,
            MemoryCommands::Info { id } => commands::memory::info(client, &id).await,
            MemoryCommands::Rm { id } => commands::memory::remove(client, &id).await,
        },
        Commands::Status => commands::status::execute(client).await,
        Commands::Start => commands::daemon::start(client).await,
        Commands::Stop => commands::daemon::stop(client).await,
        Commands::Restart => commands::daemon::restart(client).await,
        Commands::Daemon { action } => match action {
            DaemonControl::Show => {
                client.send(luci_common::IpcCommand::ShowDaemonWindow)?;
                display::print_success("Daemon window shown");
                Ok(())
            }
            DaemonControl::Hide => {
                client.send(luci_common::IpcCommand::HideDaemonWindow)?;
                display::print_success("Daemon window hidden");
                Ok(())
            }
        },
        Commands::Logs { filter, follow } => commands::logs::execute(client, filter.as_deref(), follow).await,
        Commands::Monitor { interval, node } => commands::monitor::execute(client, interval, node.as_deref()).await,
        Commands::Sync { source, target } => commands::sync_files::execute(client, &source, &target).await,
        Commands::Synced => commands::sync_files::list_synced(client).await,
        Commands::Proxy { command } => match command {
            ProxyCommands::Start { port, nodes } => commands::proxy::start(client, port, &nodes).await,
            ProxyCommands::Stop => commands::proxy::stop(client).await,
            ProxyCommands::Status => commands::proxy::status(client).await,
        },
        Commands::Failover => commands::failover::execute(client).await,
        Commands::Peers => commands::peers::execute(client).await,
        Commands::Health => commands::health::execute(client).await,
        Commands::Metrics { format } => commands::metrics::execute(client, &format).await,
    };
    result?;
    Ok(0)
}

fn first_run_setup() {
    let config = match Config::load() {
        Ok(c) => c,
        Err(_) => return,
    };
    let marker = config.data_dir.join(".setup_done");
    if marker.exists() { return; }

    println!("  {}First-time setup...{}", display::DIM, display::RESET);
    let _ = config.ensure_dirs();

    #[cfg(windows)]
    {
        let ok = setup_firewall_rules(&config);
        if ok {
            println!("  {}Firewall rules configured.{}", display::DIM, display::RESET);
        } else {
            println!("  {}Firewall rules need Administrator permission for network discovery.{}", display::YELLOW, display::RESET);
            print!("  {}Allow Luci to set up firewall rules? [Y/n]: {}", display::DIM, display::RESET);
            std::io::Write::flush(&mut std::io::stdout()).ok();
            let mut answer = String::new();
            let _ = std::io::stdin().read_line(&mut answer);
            let answer = answer.trim().to_lowercase();
            if answer.is_empty() || answer == "y" || answer == "yes" {
                if elevate_firewall_setup() {
                    println!("  {}Firewall rules configured.{}", display::DIM, display::RESET);
                } else {
                    println!("  {}Firewall setup skipped or failed. You can run luci as Administrator later.{}", display::DIM, display::RESET);
                }
            } else {
                println!("  {}Skipped. You can run luci as Administrator later to enable network discovery.{}", display::DIM, display::RESET);
            }
        }
    }

    let _ = std::fs::write(&marker, "ok");
    println!();
}

#[cfg(windows)]
fn elevate_firewall_setup() -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn ShellExecuteExW(pExecInfo: *mut SHELLEXECUTEINFOW) -> i32;
        fn WaitForSingleObject(hHandle: *mut std::ffi::c_void, dwMilliseconds: u32) -> u32;
        fn GetExitCodeProcess(hProcess: *mut std::ffi::c_void, lpExitCode: *mut u32) -> i32;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    }
    #[repr(C)]
    struct SHELLEXECUTEINFOW {
        cbSize: u32,
        fMask: u32,
        hwnd: *mut std::ffi::c_void,
        lpVerb: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShow: i32,
        hInstApp: isize,
        lpIDList: *mut std::ffi::c_void,
        lpClass: *const u16,
        hkeyClass: *mut std::ffi::c_void,
        dwHotKey: u32,
        hUnion: *mut std::ffi::c_void,
        hProcess: *mut std::ffi::c_void,
    }
    const SEE_MASK_NOCLOSEPROCESS: u32 = 0x00000040;
    const SW_HIDE: i32 = 0;
    const INFINITE: u32 = 0xFFFFFFFF;

    fn wide(s: &str) -> Vec<u16> { OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect() }
    let exe = match std::env::current_exe() { Ok(p) => p, Err(_) => return false };
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide("--setup-firewall");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: std::ptr::null_mut(),
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: params.as_ptr(),
        lpDirectory: std::ptr::null(),
        nShow: SW_HIDE,
        hInstApp: 0,
        lpIDList: std::ptr::null_mut(),
        lpClass: std::ptr::null(),
        hkeyClass: std::ptr::null_mut(),
        dwHotKey: 0,
        hUnion: std::ptr::null_mut(),
        hProcess: std::ptr::null_mut(),
    };
    unsafe {
        if ShellExecuteExW(&mut info) == 0 { return false; }
        if info.hProcess.is_null() { return false; }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code: u32 = 1;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        code == 0
    }
}

#[cfg(windows)]
fn setup_firewall_rules(config: &Config) -> bool {
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
    let mut all_ok = true;
    for (name, dir, port, proto) in &rules {
        let port_key = if *dir == "inbound" { "localport=" } else { "remoteport=" };
        let out = Command::new("netsh")
            .args(["advfirewall","firewall","add","rule","name=",name,"dir=",dir,"action=","allow","program=",&exe_str,"protocol=",proto,port_key,port,"profile=","private"])
            .output();
        match out {
            Ok(o) if o.status.success() => {}
            _ => { all_ok = false; }
        }
    }
    all_ok
}

async fn ensure_daemon_running(client: &IpcClient, port: u16) {
    if client.is_daemon_running() {
        return;
    }
    println!("  {}Starting daemon...{}", display::DIM, display::RESET);
    let _ = spawn_daemon_process(port, true);
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if client.is_daemon_running() {
            println!("  {}Daemon ready.{}", display::DIM, display::RESET);
            println!();
            return;
        }
    }
    println!("  {}Daemon not responding yet; commands may fail until it starts.{}", display::DIM, display::RESET);
    println!();
}

fn spawn_daemon_process(port: u16, hidden: bool) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--internal-daemon").arg("--port").arg(port.to_string());
    if hidden {
        cmd.arg("--hidden");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        if hidden {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn()?;
    Ok(())
}

async fn run_internal_daemon(_port: u16, hidden: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    config.ensure_dirs()?;
    let daemon = lucid::daemon::Daemon::new(config).await?;

    #[cfg(windows)]
    if hidden {
        let notify = daemon.state.read().await.shutdown_notify.clone();
        spawn_tray_icon(notify);
    }

    daemon.run().await
}

#[cfg(windows)]
fn hide_console_window() {
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
        fn ShowWindow(hwnd: *mut std::ffi::c_void, n_cmd_show: i32) -> i32;
    }
    const SW_HIDE: i32 = 0;
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(windows)]
fn spawn_tray_icon(shutdown: std::sync::Arc<tokio::sync::Notify>) {
    std::thread::spawn(move || {
        use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem, MenuEvent}};

        unsafe extern "system" {
            fn PeekMessageW(lpMsg: *mut std::ffi::c_void, hWnd: *mut std::ffi::c_void, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> i32;
            fn TranslateMessage(lpMsg: *const std::ffi::c_void) -> i32;
            fn DispatchMessageW(lpMsg: *const std::ffi::c_void) -> isize;
        }
        const PM_REMOVE: u32 = 0x0001;

        let menu = Menu::new();
        let show = MenuItem::new("Show window", true, None);
        let hide = MenuItem::new("Hide window", true, None);
        let quit = MenuItem::new("Quit daemon", true, None);
        let _ = menu.append(&show);
        let _ = menu.append(&hide);
        let _ = menu.append(&quit);
        let icon_rgba: Vec<u8> = (0..16 * 16)
            .flat_map(|i| {
                let (x, y) = (i % 16, i / 16);
                let border = x == 0 || x == 15 || y == 0 || y == 15;
                if border { [80u8, 160, 255, 255] } else { [20, 30, 50, 255] }
            })
            .collect();
        let icon = tray_icon::Icon::from_rgba(icon_rgba, 16, 16).ok();
        let mut builder = TrayIconBuilder::new()
            .with_tooltip("Luci daemon")
            .with_menu(Box::new(menu));
        if let Some(icon) = icon {
            builder = builder.with_icon(icon);
        }
        let _tray = builder.build();

        let receiver = MenuEvent::receiver();
        let mut msg = [0u8; 48];
        loop {
            unsafe {
                while PeekMessageW(msg.as_mut_ptr() as *mut _, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(msg.as_ptr() as *const _);
                    DispatchMessageW(msg.as_ptr() as *const _);
                }
            }
            if let Ok(event) = receiver.try_recv() {
                if event.id == show.id() {
                    toggle_console(true);
                } else if event.id == hide.id() {
                    toggle_console(false);
                } else if event.id == quit.id() {
                    shutdown.notify_waiters();
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

#[cfg(windows)]
fn toggle_console(show: bool) {
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
        fn ShowWindow(hwnd: *mut std::ffi::c_void, n_cmd_show: i32) -> i32;
        fn AllocConsole() -> i32;
    }
    const SW_HIDE: i32 = 0;
    const SW_SHOW: i32 = 5;
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.is_null() {
            if show { AllocConsole(); }
        } else {
            ShowWindow(hwnd, if show { SW_SHOW } else { SW_HIDE });
        }
    }
}

fn repl_commands() -> &'static [&'static str] {
    &["create","join","leave","devices","device","resources","run","jobs","job","cancel",
      "memory","status","start","stop","restart","daemon","logs","monitor","sync","synced",
      "proxy","failover","peers","health","metrics","help","exit"]
}

fn print_slash_hints(input: &str) {
    let q = input.strip_prefix('/').unwrap_or(input);
    let matches: Vec<&&str> = repl_commands().iter().filter(|c| c.starts_with(q)).collect();
    if matches.is_empty() { return; }
    let hint = matches.iter().take(8).map(|c| format!("/{}", c)).collect::<Vec<_>>().join("  ");
    println!("  {}{}{}", display::DIM, hint, display::RESET);
}

fn hint_items(buf: &str) -> (Vec<String>, Vec<String>) {
    let cmdline = buf.strip_prefix('/').unwrap_or(buf);
    let words: Vec<&str> = cmdline.split_whitespace().collect();
    let last = words.last().copied().unwrap_or("");
    let typing_flag = last.starts_with('-') || (buf.ends_with(' ') && !words.is_empty());

    // flag hints
    if !words.is_empty() && typing_flag {
        let mut cmd = Cli::command();
        for w in &words {
            let found = cmd.get_subcommands().find(|s| s.get_name() == *w || s.get_all_aliases().any(|a| a == *w)).cloned();
            if let Some(sub) = found { cmd = sub; } else { break; }
        }
        let prefix = if last.starts_with('-') { last } else { "" };
        let mut comps = Vec::new();
        let mut labels = Vec::new();
        for a in cmd.get_arguments().filter(|a| !a.is_hide_set()) {
            let long = a.get_long().map(|l| format!("--{}", l));
            let short = a.get_short().map(|s| format!("-{}", s));
            let name = match long.clone().or(short.clone()) { Some(n) => n, None => continue };
            if !prefix.is_empty() && !name.starts_with(prefix) && !short.as_ref().map_or(false, |s| s.starts_with(prefix)) { continue; }
            let help = a.get_help().map(|h| h.to_string()).unwrap_or_default();
            comps.push(long.unwrap_or(name.clone()));
            labels.push(if help.is_empty() { name } else { format!("{} ({})", name, help) });
        }
        if !comps.is_empty() {
            comps.truncate(8); labels.truncate(8);
            return (comps, labels);
        }
    }

    // command hints
    if buf.starts_with('/') {
        let q = &buf[1..];
        if !q.contains(' ') {
            let mut comps = Vec::new();
            let mut labels = Vec::new();
            for c in repl_commands().iter().filter(|c| c.starts_with(q)) {
                comps.push(format!("/{}", c));
                labels.push(format!("/{}", c));
            }
            comps.truncate(8); labels.truncate(8);
            return (comps, labels);
        }
    }
    (Vec::new(), Vec::new())
}

fn history_path() -> Option<std::path::PathBuf> {
    Config::load().ok().map(|c| c.data_dir.join("history.txt"))
}

fn load_history() -> Vec<String> {
    let Some(path) = history_path() else { return Vec::new(); };
    std::fs::read_to_string(path).unwrap_or_default().lines().map(|l| l.to_string()).filter(|l| !l.trim().is_empty()).collect()
}

fn save_history(history: &[String]) {
    let Some(path) = history_path() else { return; };
    let start = history.len().saturating_sub(100);
    let _ = std::fs::write(path, history[start..].join("\n"));
}

async fn run_repl(port: u16) -> anyhow::Result<i32> {
    println!("  {}Interactive mode. Type /help for commands, /exit to quit.{}", display::DIM, display::RESET);
    println!();
    let mut history = load_history();
    loop {
        let input = match read_repl_line(&history) {
            Some(s) => s,
            None => break,
        };
        let input = input.trim().to_string();
        if input.is_empty() { continue; }
        if history.last().map(|s| s.as_str()) != Some(&input) {
            history.push(input.clone());
            save_history(&history);
        }
        let cmdline = input.strip_prefix('/').unwrap_or(&input).trim().to_string();
        let cmdline = cmdline.strip_prefix("luci ").unwrap_or(&cmdline).trim().to_string();
        if cmdline.is_empty() || cmdline == "/" {
            print_slash_hints("/");
            continue;
        }
        if matches!(cmdline.as_str(), "exit" | "quit") {
            let client = IpcClient::new(port);
            let _ = client.send(luci_common::IpcCommand::Shutdown);
            break;
        }
        if cmdline == "help" {
            let mut cmd = Cli::command();
            let h = cmd.render_long_help().to_string()
                .replace("luci.exe [OPTIONS] [COMMAND]", "/<command> [--flags]")
                .replace("luci [OPTIONS] [COMMAND]", "/<command> [--flags]")
                .replace("luci.exe", "luci");
            println!("{}", h);
            continue;
        }
        let args = std::iter::once("luci".to_string()).chain(shell_words(&cmdline)).collect::<Vec<_>>();
        match Cli::try_parse_from(args) {
            Ok(cli) => {
                let client = IpcClient::new(port);
                if let Some(cmd) = cli.command {
                    if let Err(e) = dispatch_command(cmd, &client, port).await {
                        display::print_error(&format!("{e:#}"));
                    }
                }
            }
            Err(e) => {
                let msg = e.render().to_string()
                    .replace("luci.exe", "luci")
                    .replace("Usage: luci ", "Usage: /")
                    .replace("Usage: luci\n", "Usage: /\n")
                    .replace("try 'luci ", "try '/");
                eprint!("{}", msg);
            }
        }
    }
    Ok(0)
}

fn read_repl_line(history: &[String]) -> Option<String> {
    #[cfg(windows)]
    {
        if let Some(line) = read_repl_line_windows(history) { return Some(line); }
    }
    read_repl_line_fallback(history)
}

fn read_repl_line_fallback(_history: &[String]) -> Option<String> {
    use std::io::{self, Write};
    print!("{}luci>{} ", display::CYAN, display::RESET);
    io::stdout().flush().ok();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).ok()? == 0 { return None; }
    Some(line)
}

#[cfg(windows)]
fn read_repl_line_windows(history: &[String]) -> Option<String> {
    use std::io::Write;
    unsafe extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
        fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
        fn ReadConsoleInputW(hConsoleInput: *mut std::ffi::c_void, lpBuffer: *mut INPUT_RECORD, nLength: u32, lpNumberOfEventsRead: *mut u32) -> i32;
    }
    const STD_INPUT_HANDLE: u32 = (-10i32) as u32;
    const STD_OUTPUT_HANDLE: u32 = (-11i32) as u32;
    const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
    const ENABLE_LINE_INPUT: u32 = 0x0002;
    const ENABLE_ECHO_INPUT: u32 = 0x0004;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const KEY_EVENT: u16 = 0x0001;
    const VK_TAB: u16 = 0x09;
    const VK_LEFT: u16 = 0x25;
    const VK_UP: u16 = 0x26;
    const VK_RIGHT: u16 = 0x27;
    const VK_DOWN: u16 = 0x28;
    const SHIFT_PRESSED: u32 = 0x0010;
    const LEFT_ALT_PRESSED: u32 = 0x0002;
    const RIGHT_ALT_PRESSED: u32 = 0x0001;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct KEY_EVENT_RECORD {
        bKeyDown: i32,
        wRepeatCount: u16,
        wVirtualKeyCode: u16,
        wVirtualScanCode: u16,
        uChar: u16,
        dwControlKeyState: u32,
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    #[allow(non_snake_case)]
    struct INPUT_RECORD {
        EventType: u16,
        _pad: u16,
        Event: [u8; 16],
    }

    unsafe {
        let h = GetStdHandle(STD_INPUT_HANDLE);
        let hout = GetStdHandle(STD_OUTPUT_HANDLE);
        if h.is_null() || h == (-1isize) as *mut std::ffi::c_void { return None; }
        let mut mode: u32 = 0;
        if GetConsoleMode(h, &mut mode) == 0 { return None; }
        let mut out_mode: u32 = 0;
        if !hout.is_null() && hout != (-1isize) as *mut std::ffi::c_void && GetConsoleMode(hout, &mut out_mode) != 0 {
            SetConsoleMode(hout, out_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
        SetConsoleMode(h, mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT));

        let prompt = || {
            print!("{}luci>{} ", display::CYAN, display::RESET);
            std::io::stdout().flush().ok();
        };
        prompt();

        let mut buf = String::new();
        let mut cursor: usize = 0; // char index
        let mut sel_anchor: Option<usize> = None;
        let mut hint_labels: Vec<String> = Vec::new();
        let mut hint_comps: Vec<String> = Vec::new();
        let mut hint_sel: usize = 0;
        let mut hint_line_len: usize = 0;
        let mut history_pos: usize = history.len();
        let mut draft = String::new();
        let mut ctrl_c_armed = false;

        let redraw_input = |buf: &str, cursor: usize, sel_anchor: Option<usize>| {
            print!("\r{}luci>{} ", display::CYAN, display::RESET);
            let chars: Vec<char> = buf.chars().collect();
            if let Some(a) = sel_anchor {
                let (s, e) = if a <= cursor { (a, cursor) } else { (cursor, a) };
                for (i, ch) in chars.iter().enumerate() {
                    if i == s { print!("{}", display::REVERSE); }
                    if i == e { print!("{}", display::RESET); }
                    print!("{}", ch);
                }
                if e == chars.len() && s < e { print!("{}", display::RESET); }
            } else {
                print!("{}", buf);
            }
            print!("\x1b[K");
            let tail = chars.len().saturating_sub(cursor);
            if tail > 0 { print!("\x1b[{}D", tail); }
            std::io::stdout().flush().ok();
        };
        let clear_hint = |hint_line_len: &mut usize, buf: &str, cursor: usize, sel_anchor: Option<usize>| {
            if *hint_line_len > 0 {
                print!("\n\r{}\x1b[A", " ".repeat(*hint_line_len + 4));
                redraw_input(buf, cursor, sel_anchor);
                *hint_line_len = 0;
            }
        };
        let render_hints = |labels: &[String], sel: usize, hint_line_len: &mut usize, buf: &str, cursor: usize, sel_anchor: Option<usize>| {
            if labels.is_empty() { return; }
            let mut parts = Vec::new();
            for (i, l) in labels.iter().enumerate() {
                if i == sel {
                    parts.push(format!("{}{}{}{}", display::REVERSE, l, display::RESET, display::DIM));
                } else {
                    parts.push(l.clone());
                }
            }
            let hint = parts.join("  ");
            let visible_len: usize = labels.iter().map(|l| l.chars().count()).sum::<usize>() + labels.len().saturating_sub(1) * 2;
            print!("\n\r  {}{}{}\x1b[A", display::DIM, hint, display::RESET);
            redraw_input(buf, cursor, sel_anchor);
            *hint_line_len = visible_len + 2;
        };

        let result = loop {
            let mut rec = INPUT_RECORD { EventType: 0, _pad: 0, Event: [0; 16] };
            let mut read: u32 = 0;
            if ReadConsoleInputW(h, &mut rec, 1, &mut read) == 0 { break None; }
            if read == 0 || rec.EventType != KEY_EVENT { continue; }
            let key: KEY_EVENT_RECORD = std::mem::transmute(rec.Event);
            if key.bKeyDown == 0 { continue; }

            let vk = key.wVirtualKeyCode;
            let ch = char::from_u32(key.uChar as u32).unwrap_or('\0');
            let shift = (key.dwControlKeyState & SHIFT_PRESSED) != 0;
            let alt = (key.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED)) != 0;
            let is_ctrl_c = ch == '\x03';
            if !is_ctrl_c { ctrl_c_armed = false; }

            if vk == VK_UP {
                if !hint_labels.is_empty() {
                    hint_sel = if hint_sel == 0 { hint_labels.len() - 1 } else { hint_sel - 1 };
                    clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
                    render_hints(&hint_labels, hint_sel, &mut hint_line_len, &buf, cursor, sel_anchor);
                    continue;
                } else if history_pos > 0 {
                    if history_pos == history.len() { draft = buf.clone(); }
                    history_pos -= 1;
                    buf = history[history_pos].clone();
                    cursor = buf.chars().count();
                    sel_anchor = None;
                    clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
                    print!("\r{}luci>{} {}\x1b[K", display::CYAN, display::RESET, buf);
                    std::io::stdout().flush().ok();
                    let (c, l) = hint_items(&buf);
                    hint_comps = c; hint_labels = l; hint_sel = 0;
                    render_hints(&hint_labels, hint_sel, &mut hint_line_len, &buf, cursor, sel_anchor);
                    continue;
                }
            }
            if vk == VK_DOWN {
                if !hint_labels.is_empty() {
                    hint_sel = (hint_sel + 1) % hint_labels.len();
                    clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
                    render_hints(&hint_labels, hint_sel, &mut hint_line_len, &buf, cursor, sel_anchor);
                    continue;
                } else if history_pos < history.len() {
                    history_pos += 1;
                    buf = if history_pos == history.len() { draft.clone() } else { history[history_pos].clone() };
                    cursor = buf.chars().count();
                    sel_anchor = None;
                    clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
                    print!("\r{}luci>{} {}\x1b[K", display::CYAN, display::RESET, buf);
                    std::io::stdout().flush().ok();
                    let (c, l) = hint_items(&buf);
                    hint_comps = c; hint_labels = l; hint_sel = 0;
                    render_hints(&hint_labels, hint_sel, &mut hint_line_len, &buf, cursor, sel_anchor);
                    continue;
                }
            }
            let word_prev = |buf: &str, cursor: usize| -> usize {
                let chars: Vec<char> = buf.chars().collect();
                let mut i = cursor.saturating_sub(1);
                while i > 0 && chars[i].is_whitespace() { i -= 1; }
                while i > 0 && !chars[i-1].is_whitespace() { i -= 1; }
                i
            };
            let word_next = |buf: &str, cursor: usize| -> usize {
                let chars: Vec<char> = buf.chars().collect();
                let mut i = cursor;
                while i < chars.len() && !chars[i].is_whitespace() { i += 1; }
                while i < chars.len() && chars[i].is_whitespace() { i += 1; }
                i
            };
            if vk == VK_LEFT {
                let next = if alt { word_prev(&buf, cursor) } else { cursor.saturating_sub(1) };
                if shift { if sel_anchor.is_none() { sel_anchor = Some(cursor); } }
                else { sel_anchor = None; }
                cursor = next;
                redraw_input(&buf, cursor, sel_anchor);
                continue;
            }
            if vk == VK_RIGHT {
                let maxc = buf.chars().count();
                let next = if alt { word_next(&buf, cursor) } else { (cursor + 1).min(maxc) };
                if shift { if sel_anchor.is_none() { sel_anchor = Some(cursor); } }
                else { sel_anchor = None; }
                cursor = next;
                redraw_input(&buf, cursor, sel_anchor);
                continue;
            }
            if vk == VK_TAB && !hint_comps.is_empty() {
                let comp = hint_comps[hint_sel.min(hint_comps.len()-1)].clone();
                // replace token
                if buf.starts_with('/') && !buf[1..].contains(' ') {
                    buf = comp;
                } else {
                    // replace last flag token
                    let trimmed = buf.trim_end();
                    if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
                        buf = format!("{} {}", &trimmed[..pos], comp);
                    } else {
                        buf = comp;
                    }
                }
                cursor = buf.chars().count();
                sel_anchor = None;
                clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
                print!("\r{}luci>{} {}\x1b[K", display::CYAN, display::RESET, buf);
                std::io::stdout().flush().ok();
                let (c, l) = hint_items(&buf);
                hint_comps = c; hint_labels = l; hint_sel = 0;
                render_hints(&hint_labels, hint_sel, &mut hint_line_len, &buf, cursor, sel_anchor);
                continue;
            }

            match ch {
                '\r' | '\n' => {
                    if hint_line_len > 0 {
                        print!("\n\r{}\x1b[A", " ".repeat(hint_line_len + 4));
                        std::io::stdout().flush().ok();
                        hint_line_len = 0;
                    }
                    print!("\n");
                    std::io::stdout().flush().ok();
                    break Some(buf.clone());
                }
                '\x08' => {
                    if let Some(a) = sel_anchor.take() {
                        let (s, e) = if a <= cursor { (a, cursor) } else { (cursor, a) };
                        let mut chars: Vec<char> = buf.chars().collect();
                        chars.drain(s..e);
                        buf = chars.into_iter().collect();
                        cursor = s;
                        redraw_input(&buf, cursor, sel_anchor);
                    } else if cursor > 0 {
                        let mut chars: Vec<char> = buf.chars().collect();
                        chars.remove(cursor - 1);
                        buf = chars.into_iter().collect();
                        cursor -= 1;
                        redraw_input(&buf, cursor, sel_anchor);
                    }
                    history_pos = history.len();
                    draft.clear();
                }
                '\x01' => { // Ctrl+A
                    sel_anchor = Some(0);
                    cursor = buf.chars().count();
                    redraw_input(&buf, cursor, sel_anchor);
                }
                '\x03' => {
                    if ctrl_c_armed && buf.is_empty() {
                        print!("^C\n");
                        std::io::stdout().flush().ok();
                        break Some("/exit".to_string());
                    }
                    ctrl_c_armed = true;
                    buf.clear();
                    cursor = 0;
                    sel_anchor = None;
                    clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
                    print!("^C\n  {}Press Ctrl+C again to exit, or /exit to quit.{}\n", display::DIM, display::RESET);
                    prompt();
                    std::io::stdout().flush().ok();
                    hint_labels.clear(); hint_comps.clear(); hint_sel = 0;
                }
                c if !c.is_control() => {
                    if let Some(a) = sel_anchor.take() {
                        let (s, e) = if a <= cursor { (a, cursor) } else { (cursor, a) };
                        let mut chars: Vec<char> = buf.chars().collect();
                        chars.drain(s..e);
                        cursor = s;
                        buf = chars.into_iter().collect();
                    }
                    let mut chars: Vec<char> = buf.chars().collect();
                    chars.insert(cursor, c);
                    buf = chars.into_iter().collect();
                    cursor += 1;
                    redraw_input(&buf, cursor, sel_anchor);
                    history_pos = history.len();
                    draft.clear();
                }
                _ => { continue; }
            }

            clear_hint(&mut hint_line_len, &buf, cursor, sel_anchor);
            let (c, l) = hint_items(&buf);
            hint_comps = c; hint_labels = l; hint_sel = 0;
            render_hints(&hint_labels, hint_sel, &mut hint_line_len, &buf, cursor, sel_anchor);
        };

        SetConsoleMode(h, mode);
        if !hout.is_null() && hout != (-1isize) as *mut std::ffi::c_void { SetConsoleMode(hout, out_mode); }
        if hint_line_len > 0 {
            print!("\n\r{}\x1b[A", " ".repeat(hint_line_len + 4));
            std::io::stdout().flush().ok();
        }
        result
    }
}

fn shell_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut chars = input.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !cur.is_empty() { words.push(std::mem::take(&mut cur)); }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() { words.push(cur); }
    words
}

