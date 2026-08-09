use anyhow::Result;
use crate::ipc_client::IpcClient;
use crate::display;
use luci_common::IpcCommand;

pub struct RunConfig {
    pub script: String,
    pub args: Vec<String>,
    pub ram: Option<String>,
    pub cpu: Option<usize>,
    pub node: Option<String>,
    pub gpu: bool,
    pub vram: Option<String>,
    pub timeout: Option<u64>,
}

pub(crate) fn parse_bytes(s: &str) -> u64 {
    let s = s.trim().to_lowercase();
    if let Some(v) = s.strip_suffix("tb") {
        (v.trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0 * 1024.0 * 1024.0) as u64
    } else if let Some(v) = s.strip_suffix("gb") {
        (v.trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0 * 1024.0) as u64
    } else if let Some(v) = s.strip_suffix("mb") {
        (v.trim().parse::<f64>().unwrap_or(0.0) * 1024.0 * 1024.0) as u64
    } else if let Some(v) = s.strip_suffix("kb") {
        (v.trim().parse::<f64>().unwrap_or(0.0) * 1024.0) as u64
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}

pub(crate) fn detect_job_type(script: &str) -> String {
    if script.ends_with(".py") {
        "python".to_string()
    } else if script.ends_with(".js") || script.ends_with(".mjs") {
        "javascript".to_string()
    } else if script.ends_with(".sh") || script.ends_with(".bash") {
        "shell".to_string()
    } else if script.ends_with(".rs") {
        "rust".to_string()
    } else if script.ends_with(".go") {
        "go".to_string()
    } else {
        "shell".to_string()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

pub async fn execute(client: &IpcClient, config: RunConfig) -> Result<()> {
    display::print_banner();
    display::print_header("EXECUTING JOB");

    println!("  {}Preparing job...{}", display::DIM, display::RESET);
    println!();

    let ram_bytes = config.ram.as_deref().map(parse_bytes);
    let vram_bytes = config.vram.as_deref().map(parse_bytes);
    let job_type = detect_job_type(&config.script);

    let box_width: usize = 60;
    let inner_width = box_width - 4;

    println!("  {}{}+{}+{}", display::DIM, display::BOLD, "-".repeat(box_width), display::RESET);
    println!("  {}{}|{:^inner_width$}|{}", display::DIM, display::BOLD, "JOB CONFIGURATION", display::RESET);
    println!("  {}{}+{}+{}", display::DIM, display::BOLD, "-".repeat(box_width), display::RESET);
    println!("  {}{}|{:^inner_width$}|{}", display::DIM, display::BOLD, "", display::RESET);
    println!("  {}{}|  {}Script:{}  {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::CYAN, truncate(&config.script, inner_width - 11), display::RESET);
    println!("  {}{}|  {}Type:{}     {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, job_type, display::RESET);

    if !config.args.is_empty() {
        let args_str = config.args.join(" ");
        println!("  {}{}|  {}Args:{}     {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, truncate(&args_str, inner_width - 11), display::RESET);
    }

    if let Some(ref ram) = config.ram {
        println!("  {}{}|  {}RAM:{}       {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, ram, display::RESET);
    }

    if let Some(cpu) = config.cpu {
        println!("  {}{}|  {}CPU:{}       {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, cpu, display::RESET);
    }

    if let Some(ref node) = config.node {
        println!("  {}{}|  {}Node:{}      {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, truncate(node, inner_width - 11), display::RESET);
    }

    if config.gpu {
        println!("  {}{}|  {}GPU:{}       {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, "Required", display::RESET);
    }

    if let Some(ref vram) = config.vram {
        println!("  {}{}|  {}VRAM:{}      {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, vram, display::RESET);
    }

    if let Some(timeout) = config.timeout {
        println!("  {}{}|  {}Timeout:{}   {:<inner_width$}|{}", display::DIM, display::BOLD, display::RESET, display::DIM, display::format_duration(timeout), display::RESET);
    }

    println!("  {}{}|{:^inner_width$}|{}", display::DIM, display::BOLD, "", display::RESET);
    println!("  {}{}+{}+{}", display::DIM, display::BOLD, "-".repeat(box_width), display::RESET);
    println!();

    let response = client.send(IpcCommand::RunJob {
        name: config.script.clone(),
        job_type,
        script_path: Some(config.script),
        script_content: None,
        args: config.args,
        ram: ram_bytes,
        cpu: config.cpu,
        node: config.node,
        requires_gpu: config.gpu,
        min_vram_bytes: vram_bytes,
        max_duration_secs: config.timeout,
    })?;

    if let Some(payload) = response.payload() {
        let job_id = payload["job_id"].as_str().unwrap_or("unknown");

        display::print_success("Job submitted successfully!");
        println!();
        println!("  {}Job ID:{}   {}{}{}", display::DIM, display::RESET, display::BRIGHT_CYAN, job_id, display::RESET);
        println!();
        println!("  {}Track progress:  {}luci job {}{}", display::DIM, display::CYAN, job_id, display::RESET);
        println!();
    } else if let Some(msg) = response.error_message() {
        display::print_error(msg);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bytes_gb() {
        assert_eq!(parse_bytes("4GB"), 4 * 1024 * 1024 * 1024);
        assert_eq!(parse_bytes("1gb"), 1024 * 1024 * 1024);
        assert_eq!(parse_bytes("8.5 GB"), (8.5 * 1024.0 * 1024.0 * 1024.0) as u64);
    }

    #[test]
    fn test_parse_bytes_mb() {
        assert_eq!(parse_bytes("512MB"), 512 * 1024 * 1024);
        assert_eq!(parse_bytes("1024 mb"), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_bytes_kb() {
        assert_eq!(parse_bytes("1024KB"), 1024 * 1024);
        assert_eq!(parse_bytes("1 kb"), 1024);
    }

    #[test]
    fn test_parse_bytes_tb() {
        assert_eq!(parse_bytes("1TB"), 1024 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_bytes_raw_number() {
        assert_eq!(parse_bytes("1073741824"), 1073741824);
    }

    #[test]
    fn test_parse_bytes_invalid() {
        assert_eq!(parse_bytes("invalid"), 0);
        assert_eq!(parse_bytes(""), 0);
    }

    #[test]
    fn test_parse_bytes_whitespace() {
        assert_eq!(parse_bytes("  4 GB  "), 4 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_detect_job_type_python() {
        assert_eq!(detect_job_type("script.py"), "python");
        assert_eq!(detect_job_type("main.py"), "python");
    }

    #[test]
    fn test_detect_job_type_javascript() {
        assert_eq!(detect_job_type("app.js"), "javascript");
        assert_eq!(detect_job_type("app.mjs"), "javascript");
    }

    #[test]
    fn test_detect_job_type_shell() {
        assert_eq!(detect_job_type("run.sh"), "shell");
        assert_eq!(detect_job_type("run.bash"), "shell");
    }

    #[test]
    fn test_detect_job_type_rust() {
        assert_eq!(detect_job_type("main.rs"), "rust");
    }

    #[test]
    fn test_detect_job_type_go() {
        assert_eq!(detect_job_type("main.go"), "go");
    }

    #[test]
    fn test_detect_job_type_unknown_defaults_to_shell() {
        assert_eq!(detect_job_type("Makefile"), "shell");
        assert_eq!(detect_job_type("script.txt"), "shell");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_very_long() {
        let result = truncate("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(result, "abcdefg...");
        assert_eq!(result.len(), 10);
    }
}
