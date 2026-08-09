use std::sync::Arc;
use tokio::sync::RwLock;
use crate::daemon::DaemonState;
use crate::daemon::metrics::PrometheusMetrics;

/// HTTP server for the web dashboard
pub struct WebDashboard {
    state: Arc<RwLock<DaemonState>>,
    port: u16,
}

impl WebDashboard {
    pub fn new(state: Arc<RwLock<DaemonState>>, port: u16) -> Self {
        Self { state, port }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;
        tracing::info!("Web dashboard listening on port {}", self.port);

        loop {
            let (stream, _) = listener.accept().await?;
            let state = self.state.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, state).await {
                    tracing::error!("Dashboard connection error: {}", e);
                }
            });
        }
    }

    fn generate_dashboard_html(&self) -> String {
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Luci Dashboard</title>
    <style>
        body { font-family: monospace; background: #1a1a2e; color: #e94560; margin: 0; padding: 20px; }
        .header { text-align: center; padding: 20px; border-bottom: 2px solid #e94560; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr)); gap: 20px; margin-top: 20px; }
        .card { background: #16213e; border: 1px solid #e94560; border-radius: 8px; padding: 15px; }
        .card h3 { margin-top: 0; color: #e94560; }
        .stat { display: flex; justify-content: space-between; padding: 5px 0; border-bottom: 1px solid #333; }
        .stat:last-child { border-bottom: none; }
        .label { color: #aaa; }
        .value { color: #e94560; font-weight: bold; }
        .node { background: #0f3460; border-radius: 4px; padding: 10px; margin: 5px 0; }
        .node-name { color: #e94560; font-weight: bold; }
        .online { color: #4ecca3; }
        .offline { color: #e94560; }
        .bar { height: 20px; background: #333; border-radius: 4px; overflow: hidden; margin: 5px 0; }
        .bar-fill { height: 100%; background: #e94560; transition: width 0.3s; }
        .refresh { background: #e94560; color: #1a1a2e; border: none; padding: 10px 20px; cursor: pointer; border-radius: 4px; }
        .refresh:hover { background: #ff6b6b; }
    </style>
</head>
<body>
    <div class="header">
        <h1>LUCI DASHBOARD</h1>
        <p>4 Devices = 1 Machine</p>
        <button class="refresh" onclick="refresh()">Refresh</button>
    </div>
    <div class="grid">
        <div class="card" id="cluster-info">
            <h3>Cluster Info</h3>
            <div class="stat"><span class="label">Status:</span><span class="value" id="status">Loading...</span></div>
            <div class="stat"><span class="label">Node ID:</span><span class="value" id="node-id">-</span></div>
            <div class="stat"><span class="label">Uptime:</span><span class="value" id="uptime">-</span></div>
            <div class="stat"><span class="label">Peers:</span><span class="value" id="peers">0</span></div>
        </div>
        <div class="card" id="resources">
            <h3>Resources</h3>
            <div class="stat"><span class="label">CPU:</span><span class="value" id="cpu">-</span></div>
            <div class="bar"><div class="bar-fill" id="cpu-bar" style="width: 0%"></div></div>
            <div class="stat"><span class="label">RAM:</span><span class="value" id="ram">-</span></div>
            <div class="bar"><div class="bar-fill" id="ram-bar" style="width: 0%"></div></div>
            <div class="stat"><span class="label">GPU:</span><span class="value" id="gpu">-</span></div>
            <div class="bar"><div class="bar-fill" id="gpu-bar" style="width: 0%"></div></div>
        </div>
        <div class="card" id="jobs">
            <h3>Jobs</h3>
            <div class="stat"><span class="label">Running:</span><span class="value" id="jobs-running">0</span></div>
            <div class="stat"><span class="label">Completed:</span><span class="value" id="jobs-completed">0</span></div>
            <div class="stat"><span class="label">Failed:</span><span class="value" id="jobs-failed">0</span></div>
        </div>
        <div class="card" id="nodes">
            <h3>Connected Nodes</h3>
            <div id="nodes-list">Loading...</div>
        </div>
    </div>
    <script>
        async function refresh() {
            try {
                var resp = await fetch('/api/dashboard');
                var data = await resp.json();
                
                document.getElementById('status').textContent = data.status || 'online';
                document.getElementById('node-id').textContent = data.node_id || '-';
                document.getElementById('uptime').textContent = data.uptime || '-';
                document.getElementById('peers').textContent = data.peer_count || 0;
                
                if (data.resources) {
                    var r = data.resources;
                    document.getElementById('cpu').textContent = r.cpu_cores + ' cores / ' + r.cpu_usage + '%';
                    document.getElementById('cpu-bar').style.width = r.cpu_usage + '%';
                    document.getElementById('ram').textContent = (r.ram_used/1024/1024/1024).toFixed(1) + 'GB / ' + (r.ram_total/1024/1024/1024).toFixed(1) + 'GB';
                    document.getElementById('ram-bar').style.width = (r.ram_used/r.ram_total*100) + '%';
                    document.getElementById('gpu').textContent = r.gpu_name || 'No GPU';
                    document.getElementById('gpu-bar').style.width = (r.gpu_usage || 0) + '%';
                }
                
                if (data.jobs) {
                    document.getElementById('jobs-running').textContent = data.jobs.running || 0;
                    document.getElementById('jobs-completed').textContent = data.jobs.completed || 0;
                    document.getElementById('jobs-failed').textContent = data.jobs.failed || 0;
                }
                
                if (data.nodes) {
                    var nodesList = document.getElementById('nodes-list');
                    nodesList.innerHTML = data.nodes.map(function(n) {
                        return '<div class="node"><span class="node-name">' + n.name + '</span> <span class="' + (n.status === 'Online' ? 'online' : 'offline') + '">[' + n.status + ']</span> CPU: ' + n.cpu + '% | RAM: ' + n.ram + '</div>';
                    }).join('');
                }
            } catch (e) {
                console.error('Failed to fetch dashboard:', e);
            }
        }
        refresh();
        setInterval(refresh, 5000);
    </script>
</body>
</html>"#.to_string()
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    state: Arc<RwLock<DaemonState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buffer = vec![0; 4096];
    let n = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..n]);

    let (status, content_type, body) = if request.starts_with("GET /api/dashboard") {
        let state_guard = state.read().await;
        let dashboard = generate_api_response(&state_guard).await;
        ("200 OK", "application/json", dashboard)
    } else if request.starts_with("GET /api/nodes") {
        let state_guard = state.read().await;
        let nodes = generate_nodes_response(&state_guard).await;
        ("200 OK", "application/json", nodes)
    } else if request.starts_with("GET /api/jobs") {
        let state_guard = state.read().await;
        let jobs = generate_jobs_response(&state_guard).await;
        ("200 OK", "application/json", jobs)
    } else if request.starts_with("GET /metrics") {
        let metrics = PrometheusMetrics::new(state.clone());
        let prometheus = metrics.render().await;
        ("200 OK", "text/plain; version=0.0.4", prometheus)
    } else if request.starts_with("GET /") {
        let dashboard = WebDashboard::new(state.clone(), 0);
        ("200 OK", "text/html", dashboard.generate_dashboard_html())
    } else {
        ("404 NOT FOUND", "text/plain", "Not Found".to_string())
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
        status,
        content_type,
        body.len(),
        body
    );

    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;

    Ok(())
}

async fn generate_api_response(state: &DaemonState) -> String {
    let node = state.nodes.values().next();
    let resources = node.and_then(|n| n.resources.as_ref());
    
    let jobs = state.job_manager.list_jobs().await;
    let running = jobs.iter().filter(|j| matches!(j.status, luci_common::JobStatus::Running)).count();
    let completed = jobs.iter().filter(|j| matches!(j.status, luci_common::JobStatus::Completed)).count();
    let failed = jobs.iter().filter(|j| matches!(j.status, luci_common::JobStatus::Failed)).count();

    let nodes: Vec<serde_json::Value> = state.nodes.values().map(|n| {
        let res = n.resources.as_ref();
        serde_json::json!({
            "name": n.info.name,
            "status": format!("{:?}", n.status),
            "cpu": res.map(|r| r.cpu.load_percent).unwrap_or(0.0),
            "ram": res.map(|r| format!("{:.1}GB", r.memory.available_bytes as f64 / 1024.0 / 1024.0 / 1024.0)).unwrap_or_default(),
            "gpu": res.and_then(|r| r.gpu.as_ref()).map(|g| g.name.clone()),
        })
    }).collect();

    serde_json::json!({
        "status": "online",
        "node_id": state.nodes.keys().next().map(|k| k.to_string()).unwrap_or_default(),
        "uptime": format!("{}s", state.start_time.elapsed().as_secs()),
        "peer_count": state.nodes.len(),
        "resources": resources.map(|r| serde_json::json!({
            "cpu_cores": r.cpu.cores,
            "cpu_usage": r.cpu.load_percent,
            "ram_total": r.memory.total_bytes,
            "ram_used": r.memory.used_bytes,
            "ram_available": r.memory.available_bytes,
            "gpu_name": r.gpu.as_ref().map(|g| g.name.clone()),
            "gpu_usage": r.gpu.as_ref().map(|g| g.utilization_percent),
            "vram_total": r.gpu.as_ref().map(|g| g.vram_total_bytes),
            "vram_used": r.gpu.as_ref().map(|g| g.vram_used_bytes),
            "disk_total": r.disk.total_bytes,
            "disk_free": r.disk.free_bytes,
            "battery": r.battery_percent,
            "charging": r.is_charging,
            "temperature": r.temperature_c,
        })),
        "jobs": {
            "running": running,
            "completed": completed,
            "failed": failed,
            "total": jobs.len(),
        },
        "nodes": nodes,
    }).to_string()
}

async fn generate_nodes_response(state: &DaemonState) -> String {
    let nodes: Vec<serde_json::Value> = state.nodes.values().map(|n| {
        let res = n.resources.as_ref();
        serde_json::json!({
            "name": n.info.name,
            "node_id": n.info.node_id,
            "status": format!("{:?}", n.status),
            "ip": n.info.ip_address,
            "resources": res.map(|r| serde_json::json!({
                "cpu_cores": r.cpu.cores,
                "cpu_load": r.cpu.load_percent,
                "ram_total": r.memory.total_bytes,
                "ram_available": r.memory.available_bytes,
                "gpu": r.gpu.as_ref().map(|g| serde_json::json!({
                    "name": g.name,
                    "vram_total": g.vram_total_bytes,
                    "vram_used": g.vram_used_bytes,
                    "utilization": g.utilization_percent,
                    "temperature": g.temperature_c,
                })),
                "disk_total": r.disk.total_bytes,
                "disk_free": r.disk.free_bytes,
            })),
            "created_at": n.info.created_at,
        })
    }).collect();

    serde_json::json!({
        "nodes": nodes,
        "count": nodes.len(),
    }).to_string()
}

async fn generate_jobs_response(state: &DaemonState) -> String {
    let jobs = state.job_manager.list_jobs().await;
    let jobs_json: Vec<serde_json::Value> = jobs.iter().map(|j| {
        serde_json::json!({
            "id": j.id,
            "name": j.name,
            "type": format!("{:?}", j.job_type),
            "status": format!("{:?}", j.status),
            "node": j.assigned_node,
            "created_at": j.created_at,
            "started_at": j.started_at,
            "completed_at": j.completed_at,
            "error": j.error,
        })
    }).collect();

    serde_json::json!({
        "jobs": jobs_json,
        "count": jobs.len(),
    }).to_string()
}
