use anyhow::Result;
use luci_common::{Job, JobId, JobStatus, MetricsCollector};
use luci_runtime::{DistributedExecutor, Executor, ExecutorStatus, ExecutionResult, JobCache};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Callback type for job completion notifications
pub type JobCompletionCallback = Arc<dyn Fn(JobId, bool, String) + Send + Sync>;

pub struct JobManager {
    executor: Arc<DistributedExecutor>,
    jobs: Arc<RwLock<HashMap<JobId, Job>>>,
    results: Arc<RwLock<HashMap<JobId, ExecutionResult>>>,
    metrics: Arc<MetricsCollector>,
    persistence: Option<Arc<JobCache>>,
    completion_callback: Option<JobCompletionCallback>,
}

impl JobManager {
    pub fn new(node_id: &str, max_concurrent: usize, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            executor: Arc::new(DistributedExecutor::new(node_id.into(), max_concurrent)),
            jobs: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            metrics,
            persistence: None,
            completion_callback: None,
        }
    }

    /// Set a callback to be called when a job completes
    pub fn with_completion_callback(mut self, callback: JobCompletionCallback) -> Self {
        self.completion_callback = Some(callback);
        self
    }

    /// Clone this JobManager with a new completion callback
    pub fn clone_with_callback(&self, callback: JobCompletionCallback) -> Self {
        Self {
            executor: self.executor.clone(),
            jobs: self.jobs.clone(),
            results: self.results.clone(),
            metrics: self.metrics.clone(),
            persistence: self.persistence.clone(),
            completion_callback: Some(callback),
        }
    }

    pub fn with_persistence(mut self, persistence: JobCache) -> Self {
        self.persistence = Some(Arc::new(persistence));
        self
    }

    pub async fn submit_job(&self, mut job: Job, working_dir: &std::path::Path) -> Result<()> {
        if !self.executor.can_accept_job().await {
            return Err(anyhow::anyhow!("Executor at capacity"));
        }

        job.status = JobStatus::Running;
        job.working_dir = Some(working_dir.to_path_buf());

        // Persist job before execution
        if let Some(persistence) = &self.persistence {
            if let Err(e) = persistence.update(&job).await {
                tracing::warn!("Failed to persist job {}: {}", job.id, e);
            }
        }

        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job.id.clone(), job.clone());
        }

        self.metrics.record_job_submitted();
        self.metrics.record_job_started();

        let executor = self.executor.clone();
        let results = self.results.clone();
        let jobs_ref = self.jobs.clone();
        let job_id = job.id.clone();
        let metrics = self.metrics.clone();
        let persistence = self.persistence.clone();
        let completion_callback = self.completion_callback.clone();

        tokio::spawn(async move {
            // Apply timeout if max_duration_secs is set
            let result = if let Some(timeout_secs) = job.requirements.max_duration_secs {
                let timeout = std::time::Duration::from_secs(timeout_secs);
                match tokio::time::timeout(timeout, executor.execute(&job)).await {
                    Ok(result) => result,
                    Err(_) => {
                        tracing::warn!("Job {} timed out after {}s", job_id, timeout_secs);
                        if let Err(e) = executor.cancel(&job_id).await {
                            tracing::debug!("Failed to cancel timed-out job {}: {}", job_id, e);
                        }
                        Err(anyhow::anyhow!("Job timed out after {} seconds", timeout_secs))
                    }
                }
            } else {
                executor.execute(&job).await
            };

            match result {
                Ok(exec_result) => {
                    tracing::info!("Job {} completed successfully", job_id);
                    metrics.record_job_completed();
                    let result_output = exec_result.stdout.clone();
                    {
                        let mut jobs = jobs_ref.write().await;
                        if let Some(j) = jobs.get_mut(&job_id) {
                            j.status = JobStatus::Completed;
                            j.completed_at = Some(chrono::Utc::now());
                            j.exit_code = Some(exec_result.exit_code);
                            if exec_result.success {
                                j.result = Some(exec_result.stdout.clone());
                            } else {
                                j.error = Some(exec_result.stderr.clone());
                            }

                            // Persist completed job
                            if let Some(persist) = &persistence {
                                if let Err(e) = persist.update(j).await {
                                    tracing::warn!("Failed to persist completed job {}: {}", job_id, e);
                                }
                            }
                        }
                    }
                    let mut results = results.write().await;
                    results.insert(job_id.clone(), exec_result);

                    // Call completion callback if set
                    if let Some(callback) = &completion_callback {
                        callback(job_id, true, result_output);
                    }
                }
                Err(e) => {
                    tracing::error!("Job {} failed: {}", job_id, e);
                    metrics.record_job_failed();
                    let error_msg = e.to_string();
                    {
                        let mut jobs = jobs_ref.write().await;
                        if let Some(j) = jobs.get_mut(&job_id) {
                            j.status = JobStatus::Failed;
                            j.completed_at = Some(chrono::Utc::now());
                            j.error = Some(e.to_string());

                            // Persist failed job
                            if let Some(persist) = &persistence {
                                if let Err(e) = persist.update(j).await {
                                    tracing::warn!("Failed to persist failed job {}: {}", job_id, e);
                                }
                            }
                        }
                    }

                    // Call completion callback if set (for failures too)
                    if let Some(callback) = &completion_callback {
                        callback(job_id, false, error_msg);
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn get_job(&self, job_id: &JobId) -> Option<Job> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id).cloned()
    }

    pub async fn list_jobs(&self) -> Vec<Job> {
        let jobs = self.jobs.read().await;
        jobs.values().cloned().collect()
    }

    pub async fn cancel_job(&self, job_id: &JobId) -> Result<()> {
        self.executor.cancel(job_id).await?;
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = JobStatus::Cancelled;
        }
        Ok(())
    }

    /// Insert a job into tracking (for remote jobs received from network)
    pub async fn insert_job(&self, job: Job) {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job);
    }

    pub async fn update_job_status(&self, job_id: &JobId, status: JobStatus, message: Option<String>) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = status.clone();
            match status {
                JobStatus::Completed => {
                    job.completed_at = Some(chrono::Utc::now());
                    if let Some(msg) = message {
                        job.result = Some(msg);
                    }
                    self.metrics.record_job_completed();
                }
                JobStatus::Failed => {
                    job.completed_at = Some(chrono::Utc::now());
                    if let Some(msg) = message {
                        job.error = Some(msg);
                    }
                    self.metrics.record_job_failed();
                }
                JobStatus::Running => {
                    job.started_at = Some(chrono::Utc::now());
                }
                _ => {}
            }
            // Persist if available
            if let Some(persist) = &self.persistence {
                if let Err(e) = persist.update(job).await {
                    tracing::warn!("Failed to persist job status update {}: {}", job_id, e);
                }
            }
        }
        Ok(())
    }

    pub async fn status(&self) -> ExecutorStatus {
        self.executor.status().await
    }
}
