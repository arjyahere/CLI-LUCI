// ANSI Color Codes
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const ITALIC: &str = "\x1b[3m";
pub const REVERSE: &str = "\x1b[7m";

// Colors
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";
pub const GRAY: &str = "\x1b[90m";

// Bright Colors
pub const BRIGHT_RED: &str = "\x1b[91m";
pub const BRIGHT_GREEN: &str = "\x1b[92m";
pub const BRIGHT_YELLOW: &str = "\x1b[93m";
pub const BRIGHT_BLUE: &str = "\x1b[94m";
pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
pub const BRIGHT_CYAN: &str = "\x1b[96m";

/// Print the Luci ASCII banner
pub fn print_banner() {
    println!();
    println!("{}{}{}", CYAN, BOLD, RESET);
    println!("  ██╗     ██╗   ██╗ ██████╗██╗");
    println!("  ██║     ██║   ██║██╔════╝██║");
    println!("  ██║     ██║   ██║██║     ██║");
    println!("  ██║     ██║   ██║██║     ██║");
    println!("  ███████╗╚██████╔╝╚██████╗██║");
    println!("  ╚══════╝ ╚═════╝  ╚═════╝╚═╝{}{}", BOLD, RESET);
    println!();
    println!("  {}{}One Runtime. Infinite Resources.{}", DIM, ITALIC, RESET);
    println!();
}

/// Print a section header
pub fn print_header(title: &str) {
    println!();
    println!("{}{}{}  {}{}", BOLD, CYAN, RESET, title, RESET);
    println!("{}{}{}", DIM, "─".repeat(60), RESET);
}

/// Print a success message
pub fn print_success(msg: &str) {
    println!("  {}{}✓{}{} {}", GREEN, BOLD, RESET, GREEN, msg);
    println!("{}", RESET);
}

/// Print an error message
pub fn print_error(msg: &str) {
    println!("  {}{}✗{}{} {}", RED, BOLD, RESET, RED, msg);
    println!("{}", RESET);
}

/// Print a warning message
pub fn print_warning(msg: &str) {
    println!("  {}{}⚠{}{} {}", YELLOW, BOLD, RESET, YELLOW, msg);
    println!("{}", RESET);
}

/// Print an info message
pub fn print_info(msg: &str) {
    println!("  {}{}●{}{} {}", BLUE, BOLD, RESET, BLUE, msg);
    println!("{}", RESET);
}

/// Print a key-value pair with formatting
pub fn print_kv(key: &str, value: &str) {
    println!("  {}{:<16}{} {}{}{}", DIM, key, RESET, WHITE, value, RESET);
}

/// Print a key-value pair with color based on status
pub fn print_kv_status(key: &str, value: &str, is_ok: bool) {
    let color = if is_ok { GREEN } else { RED };
    println!("  {}{:<16}{} {}{}{}", DIM, key, RESET, color, value, RESET);
}

/// Format a status indicator
pub fn status_indicator(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "online" | "idle" | "running" | "completed" | "ok" | "healthy" => {
            format!("{}{}●{}{}", GREEN, BOLD, RESET, GREEN)
        }
        "busy" | "transferring" | "scheduled" => {
            format!("{}{}●{}{}", YELLOW, BOLD, RESET, YELLOW)
        }
        "offline" | "failed" | "error" | "dead" => {
            format!("{}{}●{}{}", RED, BOLD, RESET, RED)
        }
        _ => {
            format!("{}{}○{}{}", GRAY, BOLD, RESET, GRAY)
        }
    }
}

/// Format a progress bar
pub fn progress_bar(value: f64, max: f64, width: usize) -> String {
    let ratio = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
    let filled = (ratio * width as f64) as usize;
    let empty = width - filled;

    let color = if ratio > 0.8 {
        RED
    } else if ratio > 0.6 {
        YELLOW
    } else {
        GREEN
    };

    format!(
        "{}{}{}{}{}",
        color,
        "█".repeat(filled),
        RESET,
        "░".repeat(empty),
        RESET
    )
}

/// Format bytes to human readable
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration to human readable
pub fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Format job status with color and icon
pub fn format_job_status(status: &str) -> String {
    match status {
        "Running" => format!("{}{}▶ Running{}", GREEN, BOLD, RESET),
        "Completed" => format!("{}{}✓ Completed{}", GREEN, BOLD, RESET),
        "Failed" => format!("{}{}✗ Failed{}", RED, BOLD, RESET),
        "Cancelled" => format!("{}{}■ Cancelled{}", YELLOW, BOLD, RESET),
        "Scheduled" => format!("{}{}○ Scheduled{}", BLUE, BOLD, RESET),
        "Pending" => format!("{}{}◌ Pending{}", GRAY, BOLD, RESET),
        _ => format!("{}{}?{}", GRAY, BOLD, RESET),
    }
}

/// Format node status with icon
pub fn format_node_status(status: &str) -> String {
    match status {
        "Online" | "Idle" => format!("{}{}● Online{}", GREEN, BOLD, RESET),
        "Busy" => format!("{}{}● Busy{}", YELLOW, BOLD, RESET),
        "Offline" => format!("{}{}● Offline{}", RED, BOLD, RESET),
        _ => format!("{}{}?{}", GRAY, BOLD, RESET),
    }
}

/// Format network status with color
pub fn format_network_status(connected: bool) -> String {
    if connected {
        format!("{}{}Connected{}", GREEN, BOLD, RESET)
    } else {
        format!("{}{}Disconnected{}", RED, BOLD, RESET)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_zero() {
        let bar = progress_bar(0.0, 100.0, 10);
        assert!(bar.contains("░"));
        assert!(!bar.contains("█"));
    }

    #[test]
    fn test_progress_bar_full() {
        let bar = progress_bar(100.0, 100.0, 10);
        assert!(bar.contains("█"));
        assert!(!bar.contains("░"));
    }

    #[test]
    fn test_progress_bar_half() {
        let bar = progress_bar(50.0, 100.0, 10);
        let filled_count = bar.matches("█").count();
        let empty_count = bar.matches("░").count();
        assert_eq!(filled_count, 5);
        assert_eq!(empty_count, 5);
    }

    #[test]
    fn test_progress_bar_over_max() {
        let bar = progress_bar(200.0, 100.0, 10);
        let filled_count = bar.matches("█").count();
        assert_eq!(filled_count, 10);
    }

    #[test]
    fn test_progress_bar_negative() {
        let bar = progress_bar(-10.0, 100.0, 10);
        let empty_count = bar.matches("░").count();
        assert_eq!(empty_count, 10);
    }

    #[test]
    fn test_progress_bar_zero_max() {
        let bar = progress_bar(0.0, 0.0, 10);
        let empty_count = bar.matches("░").count();
        assert_eq!(empty_count, 10);
    }

    #[test]
    fn test_format_bytes_b() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn test_format_bytes_kb() {
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
    }

    #[test]
    fn test_format_bytes_mb() {
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn test_format_bytes_gb() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_format_bytes_tb() {
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1.0 TB");
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(30), "30s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(90), "1m 30s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(3661), "1h 1m 1s");
    }

    #[test]
    fn test_format_duration_days() {
        assert_eq!(format_duration(90000), "1d 1h 0m");
    }

    #[test]
    fn test_format_job_status() {
        assert!(format_job_status("Running").contains("Running"));
        assert!(format_job_status("Completed").contains("Completed"));
        assert!(format_job_status("Failed").contains("Failed"));
        assert!(format_job_status("Cancelled").contains("Cancelled"));
        assert!(format_job_status("Unknown").contains("?"));
    }

    #[test]
    fn test_format_node_status() {
        assert!(format_node_status("Online").contains("Online"));
        assert!(format_node_status("Idle").contains("Online"));
        assert!(format_node_status("Busy").contains("Busy"));
        assert!(format_node_status("Offline").contains("Offline"));
        assert!(format_node_status("Unknown").contains("?"));
    }

    #[test]
    fn test_format_network_status() {
        assert!(format_network_status(true).contains("Connected"));
        assert!(format_network_status(false).contains("Disconnected"));
    }

    #[test]
    fn test_status_indicator() {
        assert!(status_indicator("online").contains("●"));
        assert!(status_indicator("busy").contains("●"));
        assert!(status_indicator("offline").contains("●"));
        assert!(status_indicator("unknown").contains("○"));
    }

    #[test]
    fn test_print_kv() {
        print_kv("Test Key", "test value");
    }

    #[test]
    fn test_print_kv_status_ok() {
        print_kv_status("Status", "healthy", true);
    }

    #[test]
    fn test_print_kv_status_error() {
        print_kv_status("Status", "failed", false);
    }
}
