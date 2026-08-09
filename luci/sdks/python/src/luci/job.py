"""
Luci Job - Job management.
"""

from dataclasses import dataclass
from typing import Optional
from datetime import datetime


@dataclass
class JobResult:
    """Result of a job execution."""
    job_id: str
    success: bool
    exit_code: int
    stdout: str
    stderr: str
    duration_ms: int


class Job:
    """Represents a Luci job."""

    def __init__(self, client, job_id: str, name: str):
        """
        Initialize a job.

        Args:
            client: LuciClient instance
            job_id: Job ID
            name: Job name
        """
        self._client = client
        self.id = job_id
        self.name = name

    @classmethod
    def submit(cls, client, script: str, name: Optional[str] = None, **kwargs) -> "Job":
        """
        Submit a job.

        Args:
            client: LuciClient instance
            script: Script path or command
            name: Job name (optional)
            **kwargs: Additional arguments (ram, cpu, node)

        Returns:
            Job instance
        """
        job_id = client.run(script, **kwargs)
        return cls(client, job_id, name or script)

    def status(self) -> dict:
        """Get job status."""
        return self._client.job_status(self.id)

    def wait(self, timeout: Optional[float] = None) -> JobResult:
        """
        Wait for job to complete.

        Args:
            timeout: Timeout in seconds (optional)

        Returns:
            JobResult
        """
        import time

        start = time.time()
        while True:
            status = self.status()
            if status.get("status") in ("completed", "failed", "cancelled"):
                return JobResult(
                    job_id=self.id,
                    success=status.get("status") == "completed",
                    exit_code=status.get("exit_code", -1),
                    stdout=status.get("result", ""),
                    stderr=status.get("error", ""),
                    duration_ms=0,
                )

            if timeout and (time.time() - start) > timeout:
                raise TimeoutError(f"Job {self.id} timed out")

            time.sleep(0.1)

    def cancel(self) -> bool:
        """Cancel the job."""
        return self._client.cancel_job(self.id)

    def __repr__(self) -> str:
        return f"Job(id={self.id!r}, name={self.name!r})"
