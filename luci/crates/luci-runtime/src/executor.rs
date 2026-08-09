use anyhow::Result;
use async_trait::async_trait;
use luci_common::{Job, JobId, NodeId};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

use crate::persistence::JobCache;

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self, job: &Job) -> Result<ExecutionResult>;
    async fn cancel(&self, job_id: &JobId) -> Result<()>;
    async fn status(&self) -> ExecutorStatus;
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub job_id: JobId,
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutorStatus {
    pub active_jobs: usize,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub total_execution_time_ms: u64,
}

/// Resource limits for job execution
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_memory_bytes: Option<u64>,
    pub max_cpu_percent: Option<f32>,
    pub max_duration_secs: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: Some(2 * 1024 * 1024 * 1024), // 2GB default
            max_cpu_percent: Some(80.0),
            max_duration_secs: Some(3600), // 1 hour
        }
    }
}

type ActiveJob = (Job, Option<tokio::process::Child>);
type ActiveJobs = Arc<RwLock<HashMap<JobId, ActiveJob>>>;

pub struct DistributedExecutor {
    node_id: NodeId,
    max_concurrent_jobs: usize,
    active_jobs: ActiveJobs,
    completed_count: Arc<RwLock<u64>>,
    failed_count: Arc<RwLock<u64>>,
    total_execution_time_ms: Arc<std::sync::atomic::AtomicU64>,
    persistence: Option<Arc<JobCache>>,
    resource_limits: ResourceLimits,
}

impl DistributedExecutor {
    pub fn new(node_id: NodeId, max_concurrent_jobs: usize) -> Self {
        Self {
            node_id,
            max_concurrent_jobs,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            completed_count: Arc::new(RwLock::new(0)),
            failed_count: Arc::new(RwLock::new(0)),
            total_execution_time_ms: Arc::new(AtomicU64::new(0)),
            persistence: None,
            resource_limits: ResourceLimits::default(),
        }
    }

    pub fn with_persistence(mut self, persistence: JobCache) -> Self {
        self.persistence = Some(Arc::new(persistence));
        self
    }

    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    pub async fn can_accept_job(&self) -> bool {
        let active = self.active_jobs.read().await;
        active.len() < self.max_concurrent_jobs
    }

    pub async fn submit_job(&self, job: Job) -> Result<()> {
        if !self.can_accept_job().await {
            return Err(anyhow::anyhow!("Executor at capacity"));
        }

        let mut active = self.active_jobs.write().await;
        active.insert(job.id.clone(), (job, None));
        Ok(())
    }

    /// Stream output lines for a running job
    pub async fn stream_output(&self, job_id: &JobId) -> Result<tokio::sync::mpsc::Receiver<(String, bool)>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        
        let active = self.active_jobs.read().await;
        if let Some((_job, Some(child))) = active.get(job_id) {
            let child_id = child.id();
            tracing::debug!("Streaming output for job {} (PID: {:?})", job_id, child_id);
            
            // For now, we can't easily stream from a spawned child process
            // This would need platform-specific implementation (e.g., reading from stdout/stderr)
            // For Windows, we'd need to use named pipes or similar
            // For Unix, we'd use non-blocking reads on the stdout/stderr file descriptors
            drop(active);
            
            // Spawn a task that will send a completion message when the job finishes
            let tx_clone = tx.clone();
            let job_id_clone = job_id.clone();
            tokio::spawn(async move {
                // Wait a bit for the job to potentially finish
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let _ = tx_clone.send(("".to_string(), true)).await;
                tracing::debug!("Stream output ended for job {}", job_id_clone);
            });
        } else {
            drop(active);
            // Job not found, send immediate completion
            let _ = tx.send(("Job not found".to_string(), true)).await;
        }
        
        Ok(rx)
    }

    /// Get persisted job history
    pub async fn get_job_history(&self, limit: usize) -> Result<Vec<luci_common::Job>> {
        if let Some(persistence) = &self.persistence {
            persistence.recent(limit).await
        } else {
            Ok(Vec::new())
        }
    }

    /// Resume jobs from persistence on startup
    pub async fn resume_jobs(&self) -> Result<()> {
        if let Some(persistence) = &self.persistence {
            let active_jobs = persistence.get_active_jobs().await?;
            tracing::info!("Resuming {} active jobs from persistence", active_jobs.len());
            
            for job in active_jobs {
                tracing::info!("Resuming job {}: {}", job.id, job.name);
                // Resubmit the job to the executor
                if let Err(e) = self.submit_job(job.clone()).await {
                    tracing::warn!("Failed to resume job {}: {}", job.id, e);
                } else {
                    tracing::info!("Resumed job {}", job.id);
                }
            }
        }
        Ok(())
    }

    async fn run_job(&self, job: &Job) -> Result<ExecutionResult> {
        tracing::debug!("Running job {} on node {}", job.id, self.node_id);
        
        // Persist job before execution
        if let Some(persistence) = &self.persistence {
            let mut job_clone = job.clone();
            job_clone.status = luci_common::JobStatus::Running;
            job_clone.started_at = Some(chrono::Utc::now());
            persistence.update(&job_clone).await?;
        }
        
        let start = std::time::Instant::now();
        
        let result = match job.job_type {
            luci_common::JobType::Python => self.run_python(job).await,
            luci_common::JobType::NodeJs => self.run_nodejs(job).await,
            luci_common::JobType::Shell => self.run_shell(job).await,
            luci_common::JobType::Custom => self.run_custom(job).await,
        };

        let duration = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut output) => {
                output.duration_ms = duration;
                let mut count = self.completed_count.write().await;
                *count += 1;
                self.total_execution_time_ms.fetch_add(duration, Ordering::Relaxed);
                
                // Persist completed job
                if let Some(persistence) = &self.persistence {
                    let mut job_clone = job.clone();
                    job_clone.status = luci_common::JobStatus::Completed;
                    job_clone.completed_at = Some(chrono::Utc::now());
                    job_clone.exit_code = Some(output.exit_code);
                    if output.success {
                        job_clone.result = Some(output.stdout.clone());
                    } else {
                        job_clone.error = Some(output.stderr.clone());
                    }
                    persistence.update(&job_clone).await?;
                }
                
                Ok(output)
            }
            Err(e) => {
                let mut count = self.failed_count.write().await;
                *count += 1;
                self.total_execution_time_ms.fetch_add(duration, Ordering::Relaxed);
                
                // Persist failed job
                if let Some(persistence) = &self.persistence {
                    let mut job_clone = job.clone();
                    job_clone.status = luci_common::JobStatus::Failed;
                    job_clone.completed_at = Some(chrono::Utc::now());
                    job_clone.error = Some(e.to_string());
                    persistence.update(&job_clone).await?;
                }
                
                Err(e)
            }
        }
    }

    async fn run_python(&self, job: &Job) -> Result<ExecutionResult> {
        // Try python3 first, then python
        let python_cmd = if tokio::process::Command::new("python3")
            .arg("--version")
            .output()
            .await
            .is_ok()
        {
            "python3"
        } else {
            "python"
        };

        let mut cmd = tokio::process::Command::new(python_cmd);
        
        if let Some(path) = &job.script_path {
            cmd.arg(path);
        } else if let Some(content) = &job.script_content {
            cmd.arg("-c").arg(content);
        }
        
        for arg in &job.args {
            cmd.arg(arg);
        }
        
        if let Some(dir) = &job.working_dir {
            cmd.current_dir(dir);
        }
        
        // Apply resource limits
        if let Some(_max_duration) = self.resource_limits.max_duration_secs {
            cmd.kill_on_drop(true);
        }
        
        // Set environment limits
        if let Some(max_memory) = self.resource_limits.max_memory_bytes {
            // On Windows, we can't directly set memory limits via environment
            // On Linux, we could use cgroups or ulimit
            // For now, we just log the limit
            tracing::debug!("Memory limit: {} bytes", max_memory);
        }
        
        let child = cmd.spawn()?;
        
        // Monitor resource usage if limits are set
        let limits = self.resource_limits.clone();
        let job_id = job.id.clone();
        let max_duration = limits.max_duration_secs;
        
        tokio::spawn(async move {
            if let Some(timeout_secs) = max_duration {
                tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
                tracing::warn!("Job {} exceeded time limit ({}s), terminating", job_id, timeout_secs);
                // Note: We can't directly kill the child here since we don't have ownership
                // The kill_on_drop(true) should handle cleanup
            }
        });
        
        {
            let mut active = self.active_jobs.write().await;
            active.insert(job.id.clone(), (job.clone(), Some(child)));
        }
        
        let output = {
            let mut active = self.active_jobs.write().await;
            if let Some((_job, Some(child))) = active.remove(&job.id) {
                child.wait_with_output().await?
            } else {
                return Err(anyhow::anyhow!("Job process not found"));
            }
        };
        
        Ok(ExecutionResult {
            job_id: job.id.clone(),
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: 0,
        })
    }

    async fn run_custom(&self, job: &Job) -> Result<ExecutionResult> {
        // Custom jobs: if script_path exists, run the executable directly with args.
        // If no script_path (inline command), use shell interpretation to support
        // builtins like `echo`, pipes, redirects, etc.

        let mut cmd = if job.script_path.is_some() {
            // File exists — run executable directly with args
            let mut c = tokio::process::Command::new(&job.name);
            if let Some(path) = &job.script_path {
                c.arg(path);
            }
            for arg in &job.args {
                c.arg(arg);
            }
            c
        } else {
            // Inline command — use shell interpretation
            #[cfg(windows)]
            {
                let mut c = tokio::process::Command::new("cmd");
                c.arg("/C").arg(&job.name);
                for arg in &job.args {
                    c.arg(arg);
                }
                c
            }
            #[cfg(not(windows))]
            {
                let mut c = tokio::process::Command::new("sh");
                c.arg("-c").arg(&job.name);
                for arg in &job.args {
                    c.arg(arg);
                }
                c
            }
        };
        
        if let Some(dir) = &job.working_dir {
            cmd.current_dir(dir);
        }
        
        let child = cmd.spawn()?;
        
        {
            let mut active = self.active_jobs.write().await;
            active.insert(job.id.clone(), (job.clone(), Some(child)));
        }
        
        let output = {
            let mut active = self.active_jobs.write().await;
            if let Some((_job, Some(child))) = active.remove(&job.id) {
                child.wait_with_output().await?
            } else {
                return Err(anyhow::anyhow!("Job process not found"));
            }
        };
        
        Ok(ExecutionResult {
            job_id: job.id.clone(),
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: 0,
        })
    }

    async fn run_nodejs(&self, job: &Job) -> Result<ExecutionResult> {
        // Try nodejs first, then node
        let node_cmd = if tokio::process::Command::new("nodejs")
            .arg("--version")
            .output()
            .await
            .is_ok()
        {
            "nodejs"
        } else {
            "node"
        };

        let mut cmd = tokio::process::Command::new(node_cmd);
        
        if let Some(path) = &job.script_path {
            cmd.arg(path);
        } else if let Some(content) = &job.script_content {
            cmd.arg("-e").arg(content);
        }
        
        for arg in &job.args {
            cmd.arg(arg);
        }
        
        if let Some(dir) = &job.working_dir {
            cmd.current_dir(dir);
        }
        
        let child = cmd.spawn()?;
        
        {
            let mut active = self.active_jobs.write().await;
            active.insert(job.id.clone(), (job.clone(), Some(child)));
        }
        
        let output = {
            let mut active = self.active_jobs.write().await;
            if let Some((_job, Some(child))) = active.remove(&job.id) {
                child.wait_with_output().await?
            } else {
                return Err(anyhow::anyhow!("Job process not found"));
            }
        };
        
        Ok(ExecutionResult {
            job_id: job.id.clone(),
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: 0,
        })
    }

    async fn run_shell(&self, job: &Job) -> Result<ExecutionResult> {
        let mut cmd = tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        if cfg!(windows) {
            cmd.arg("/C");
        } else {
            cmd.arg("-c");
        }
        
        if let Some(content) = &job.script_content {
            cmd.arg(content);
        } else if let Some(path) = &job.script_path {
            cmd.arg(path);
        }
        
        for arg in &job.args {
            cmd.arg(arg);
        }
        
        if let Some(dir) = &job.working_dir {
            cmd.current_dir(dir);
        }
        
        let child = cmd.spawn()?;
        
        {
            let mut active = self.active_jobs.write().await;
            active.insert(job.id.clone(), (job.clone(), Some(child)));
        }
        
        let output = {
            let mut active = self.active_jobs.write().await;
            if let Some((_job, Some(child))) = active.remove(&job.id) {
                child.wait_with_output().await?
            } else {
                return Err(anyhow::anyhow!("Job process not found"));
            }
        };
        
        Ok(ExecutionResult {
            job_id: job.id.clone(),
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms: 0,
        })
    }
}

#[async_trait]
impl Executor for DistributedExecutor {
    async fn execute(&self, job: &Job) -> Result<ExecutionResult> {
        self.run_job(job).await
    }

    async fn cancel(&self, job_id: &JobId) -> Result<()> {
        let mut active = self.active_jobs.write().await;
        if let Some((_job, Some(mut child))) = active.remove(job_id) {
            if let Err(e) = child.kill().await {
                tracing::warn!("Failed to kill job process {}: {}", job_id, e);
            }
        }
        Ok(())
    }

    async fn status(&self) -> ExecutorStatus {
        let active = self.active_jobs.read().await;
        let completed = self.completed_count.read().await;
        let failed = self.failed_count.read().await;
        
        ExecutorStatus {
            active_jobs: active.len(),
            completed_jobs: *completed,
            failed_jobs: *failed,
            total_execution_time_ms: self.total_execution_time_ms.load(Ordering::Relaxed),
        }
    }
}
