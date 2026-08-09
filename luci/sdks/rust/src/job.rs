use anyhow::Result;
use crate::LuciClient;

/// JobResult - Result of a job execution.
#[derive(Debug, Clone)]
pub struct JobResult {
    pub job_id: String,
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// Job - Represents a Luci job.
#[derive(Debug, Clone)]
pub struct Job {
    client: LuciClient,
    pub id: String,
    pub name: String,
}

impl Job {
    /// Submit a job.
    pub fn submit(
        client: LuciClient,
        script: &str,
        args: Vec<String>,
        ram: Option<u64>,
        cpu: Option<usize>,
        node: Option<String>,
    ) -> Result<Self> {
        let job_id = client.run(script, args, ram, cpu, node)?;
        Ok(Self {
            client,
            id: job_id,
            name: script.to_string(),
        })
    }

    /// Get job status.
    pub fn status(&self) -> Result<serde_json::Value> {
        self.client.job_status(&self.id)
    }

    /// Wait for job to complete.
    pub fn wait(&self, timeout_ms: Option<u64>) -> Result<JobResult> {
        let start = std::time::Instant::now();
        let timeout = timeout_ms.map(std::time::Duration::from_millis);

        loop {
            let status = self.status()?;
            let status_str = status["status"].as_str().unwrap_or("");

            if matches!(status_str, "completed" | "failed" | "cancelled") {
                return Ok(JobResult {
                    job_id: self.id.clone(),
                    success: status_str == "completed",
                    exit_code: status["exit_code"].as_i64().unwrap_or(-1) as i32,
                    stdout: status["result"].as_str().unwrap_or("").to_string(),
                    stderr: status["error"].as_str().unwrap_or("").to_string(),
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }

            if let Some(timeout) = timeout {
                if start.elapsed() > timeout {
                    return Err(anyhow::anyhow!("Job {} timed out", self.id));
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// Cancel the job.
    pub fn cancel(&self) -> Result<()> {
        self.client.cancel_job(&self.id)
    }
}

impl std::fmt::Display for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Job(id={}, name={})", self.id, self.name)
    }
}
