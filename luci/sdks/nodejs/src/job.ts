import { LuciClient, JobStatus } from './client';

export interface JobResult {
  jobId: string;
  success: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  durationMs: number;
}

/**
 * Job - Represents a Luci job.
 */
export class Job {
  private client: LuciClient;
  public id: string;
  public name: string;

  constructor(client: LuciClient, id: string, name: string) {
    this.client = client;
    this.id = id;
    this.name = name;
  }

  /**
   * Submit a job.
   */
  static async submit(
    client: LuciClient,
    script: string,
    options: {
      name?: string;
      args?: string[];
      ram?: string;
      cpu?: number;
      node?: string;
    } = {}
  ): Promise<Job> {
    const jobId = await client.run(script, options);
    return new Job(client, jobId, options.name || script);
  }

  /**
   * Get job status.
   */
  async status(): Promise<JobStatus> {
    return this.client.jobStatus(this.id);
  }

  /**
   * Wait for job to complete.
   */
  async wait(timeoutMs?: number): Promise<JobResult> {
    const start = Date.now();

    while (true) {
      const status = await this.status();

      if (['completed', 'failed', 'cancelled'].includes(status.status)) {
        return {
          jobId: this.id,
          success: status.status === 'completed',
          exitCode: status.exit_code ?? -1,
          stdout: status.result ?? '',
          stderr: status.error ?? '',
          durationMs: 0,
        };
      }

      if (timeoutMs && Date.now() - start > timeoutMs) {
        throw new Error(`Job ${this.id} timed out`);
      }

      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }

  /**
   * Cancel the job.
   */
  async cancel(): Promise<boolean> {
    return this.client.cancelJob(this.id);
  }

  toString(): string {
    return `Job(id=${this.id}, name=${this.name})`;
  }
}
