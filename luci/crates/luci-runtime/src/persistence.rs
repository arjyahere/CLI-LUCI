use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use luci_common::{Job, JobId, JobStatus};
use sled::Db;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Persistent job storage using sled embedded database
pub struct JobPersistence {
    db: Arc<Db>,
    jobs_tree: sled::Tree,
    results_tree: sled::Tree,
}

impl JobPersistence {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("jobs").join("sled");
        std::fs::create_dir_all(&db_path)
            .with_context(|| format!("Failed to create job DB directory: {:?}", db_path))?;

        let db = sled::open(&db_path)
            .with_context(|| format!("Failed to open sled database at {:?}", db_path))?;

        let jobs_tree = db.open_tree("jobs")
            .context("Failed to open jobs tree")?;
        let results_tree = db.open_tree("results")
            .context("Failed to open results tree")?;

        tracing::info!("Job persistence initialized at {:?}", db_path);

        Ok(Self {
            db: Arc::new(db),
            jobs_tree,
            results_tree,
        })
    }

    /// Persist a job to disk
    pub fn save_job(&self, job: &Job) -> Result<()> {
        let key = job.id.as_str().as_bytes();
        let value = serde_json::to_vec(job)
            .context("Failed to serialize job")?;
        self.jobs_tree.insert(key, value)
            .context("Failed to insert job into sled")?;
        self.db.flush().context("Failed to flush job DB")?;
        Ok(())
    }

    /// Load a job from disk
    pub fn load_job(&self, job_id: &JobId) -> Result<Option<Job>> {
        let key = job_id.as_str().as_bytes();
        match self.jobs_tree.get(key)
            .context("Failed to query job from sled")? {
            Some(value) => {
                let job: Job = serde_json::from_slice(&value)
                    .context("Failed to deserialize job")?;
                Ok(Some(job))
            }
            None => Ok(None),
        }
    }

    /// Save execution result
    pub fn save_result(&self, job_id: &JobId, result: &PersistentExecutionResult) -> Result<()> {
        let key = job_id.as_str().as_bytes();
        let value = serde_json::to_vec(result)
            .context("Failed to serialize execution result")?;
        self.results_tree.insert(key, value)
            .context("Failed to insert result into sled")?;
        self.db.flush().context("Failed to flush results DB")?;
        Ok(())
    }

    /// Load execution result
    pub fn load_result(&self, job_id: &JobId) -> Result<Option<PersistentExecutionResult>> {
        let key = job_id.as_str().as_bytes();
        match self.results_tree.get(key)
            .context("Failed to query result from sled")? {
            Some(value) => {
                let result: PersistentExecutionResult = serde_json::from_slice(&value)
                    .context("Failed to deserialize execution result")?;
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    /// List all persisted jobs with optional status filter
    pub fn list_jobs(&self, status_filter: Option<JobStatus>) -> Result<Vec<Job>> {
        let mut jobs = Vec::new();

        for entry in self.jobs_tree.iter() {
            let (_, value) = entry.context("Failed to read job entry")?;
            let job: Job = serde_json::from_slice(&value)
                .context("Failed to deserialize job")?;

            if let Some(ref status) = status_filter {
                if job.status != *status {
                    continue;
                }
            }

            jobs.push(job);
        }

        Ok(jobs)
    }

    /// Delete a job and its result
    pub fn delete_job(&self, job_id: &JobId) -> Result<()> {
        let key = job_id.as_str().as_bytes();
        self.jobs_tree.remove(key)
            .context("Failed to remove job from sled")?;
        self.results_tree.remove(key)
            .context("Failed to remove result from sled")?;
        self.db.flush().context("Failed to flush after delete")?;
        Ok(())
    }

    /// Get count of jobs by status
    pub fn count_by_status(&self) -> Result<std::collections::HashMap<String, usize>> {
        let mut counts = std::collections::HashMap::new();

        for entry in self.jobs_tree.iter() {
            let (_, value) = entry.context("Failed to read job entry")?;
            let job: Job = serde_json::from_slice(&value)
                .context("Failed to deserialize job")?;

            let status_key = format!("{:?}", job.status);
            *counts.entry(status_key).or_insert(0) += 1;
        }

        Ok(counts)
    }

    /// Get total disk usage in bytes
    pub fn disk_usage(&self) -> Result<u64> {
        self.db.size_on_disk()
            .context("Failed to get DB size")
    }

    /// Compact the database
    pub fn compact(&self) -> Result<()> {
        self.db.flush()
            .context("Failed to flush before compaction")?;
        // sled auto-compacts, but we can trigger a flush
        Ok(())
    }

    /// Get active (running/pending) jobs
    pub fn get_active_jobs(&self) -> Result<Vec<Job>> {
        let mut active = Vec::new();

        for entry in self.jobs_tree.iter() {
            let (_, value) = entry.context("Failed to read job entry")?;
            let job: Job = serde_json::from_slice(&value)
                .context("Failed to deserialize job")?;

            if matches!(job.status, JobStatus::Running | JobStatus::Pending) {
                active.push(job);
            }
        }

        Ok(active)
    }

    /// Get recent jobs (most recent first)
    pub fn get_recent_jobs(&self, limit: usize) -> Result<Vec<Job>> {
        let mut jobs = self.list_jobs(None)?;
        jobs.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        jobs.truncate(limit);
        Ok(jobs)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistentExecutionResult {
    pub job_id: JobId,
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub completed_at: DateTime<Utc>,
}

/// In-memory job cache with disk persistence
#[derive(Clone)]
pub struct JobCache {
    persistence: Arc<JobPersistence>,
    cache: Arc<RwLock<std::collections::HashMap<JobId, Job>>>,
}

impl JobCache {
    pub fn new(persistence: JobPersistence) -> Self {
        let mut cache = std::collections::HashMap::new();

        // Load active jobs into memory on startup
        if let Ok(active_jobs) = persistence.get_active_jobs() {
            for job in active_jobs {
                cache.insert(job.id.clone(), job);
            }
        }

        Self {
            persistence: Arc::new(persistence),
            cache: Arc::new(RwLock::new(cache)),
        }
    }

    /// Get job from cache or disk
    pub async fn get(&self, job_id: &JobId) -> Result<Option<Job>> {
        {
            let cache = self.cache.read().await;
            if let Some(job) = cache.get(job_id) {
                return Ok(Some(job.clone()));
            }
        }

        // Load from disk
        let job = self.persistence.load_job(job_id)?;
        if let Some(job) = &job {
            let mut cache = self.cache.write().await;
            cache.insert(job_id.clone(), job.clone());
        }

        Ok(job)
    }

    /// Update job in cache and disk
    pub async fn update(&self, job: &Job) -> Result<()> {
        self.persistence.save_job(job)?;
        let mut cache = self.cache.write().await;
        cache.insert(job.id.clone(), job.clone());
        Ok(())
    }

    /// Remove job from cache and disk
    pub async fn remove(&self, job_id: &JobId) -> Result<()> {
        self.persistence.delete_job(job_id)?;
        let mut cache = self.cache.write().await;
        cache.remove(job_id);
        Ok(())
    }

    /// Get all jobs (cache + disk)
    pub async fn list(&self) -> Result<Vec<Job>> {
        self.persistence.list_jobs(None)
    }

    /// Get recent jobs
    pub async fn recent(&self, limit: usize) -> Result<Vec<Job>> {
        self.persistence.get_recent_jobs(limit)
    }

    /// Get active jobs from persistence
    pub async fn get_active_jobs(&self) -> Result<Vec<Job>> {
        self.persistence.get_active_jobs()
    }
}