import * as net from 'net';

export interface DaemonStatus {
  running: boolean;
  pid: number;
  uptime_secs: number;
  node_id: string;
  network_id: string | null;
  connected_nodes: number;
  active_jobs: number;
  total_jobs_completed: number;
}

export interface Device {
  node_id: string;
  name: string;
  status: string;
  cpu_cores: number;
  cpu_load: number;
  ram_total_gb: number;
  ram_available_gb: number;
  gpu_name: string | null;
  vram_total_gb: number | null;
}

export interface Resources {
  devices: number;
  cpu_cores: number;
  ram_total_gb: string;
  ram_available_gb: string;
  gpu_count: number;
  vram_total_gb: string;
}

export interface JobStatus {
  id: string;
  name: string;
  type: string;
  status: string;
  assigned_node: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  result: string | null;
  error: string | null;
  exit_code: number | null;
}

export interface MemoryPoolInfo {
  pool_id: string;
  name: string;
  total_size_gb: number;
}

/**
 * Luci Client - Interface to the Luci daemon.
 */
export class LuciClient {
  private host: string;
  private port: number;

  constructor(host: string = '127.0.0.1', port: number = 9527) {
    this.host = host;
    this.port = port;
  }

  private async sendCommand(command: Record<string, unknown>): Promise<any> {
    return new Promise((resolve, reject) => {
      const sock = new net.Socket();
      sock.setTimeout(30000);

      sock.connect(this.port, this.host, () => {
        const request = JSON.stringify({ command });
        const length = Buffer.alloc(4);
        length.writeUInt32LE(Buffer.byteLength(request));
        sock.write(length);
        sock.write(request);
      });

      let buffer = Buffer.alloc(0);
      let expectedLength: number | null = null;

      sock.on('data', (data: Buffer) => {
        buffer = Buffer.concat([buffer, data]);

        if (expectedLength === null && buffer.length >= 4) {
          expectedLength = buffer.readUInt32LE(0);
          buffer = buffer.slice(4);
        }

        if (expectedLength !== null && buffer.length >= expectedLength) {
          const responseJson = buffer.slice(0, expectedLength).toString();
          const response = JSON.parse(responseJson);
          sock.destroy();
          resolve(response);
        }
      });

      sock.on('error', (err) => {
        reject(err);
      });

      sock.on('timeout', () => {
        sock.destroy();
        reject(new Error('Connection timeout'));
      });
    });
  }

  /**
   * Check if daemon is running.
   */
  async ping(): Promise<boolean> {
    try {
      const response = await this.sendCommand({ type: 'Ping' });
      return response.type === 'Success';
    } catch {
      return false;
    }
  }

  /**
   * Get daemon status.
   */
  async status(): Promise<DaemonStatus> {
    const response = await this.sendCommand({ type: 'GetDaemonStatus' });
    if (response.type === 'Success') {
      return response.payload;
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Create a new network.
   */
  async createNetwork(name: string): Promise<{ network_id: string; invite_code: string }> {
    const response = await this.sendCommand({
      type: 'CreateNetwork',
      name,
    });
    if (response.type === 'Success') {
      return response.payload;
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Join an existing network.
   */
  async joinNetwork(inviteCode: string): Promise<boolean> {
    const response = await this.sendCommand({
      type: 'JoinNetwork',
      invite_code: inviteCode,
    });
    return response.type === 'Success';
  }

  /**
   * List all devices.
   */
  async listDevices(): Promise<Device[]> {
    const response = await this.sendCommand({ type: 'ListDevices' });
    if (response.type === 'Success') {
      return response.payload.devices || [];
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Get combined resources.
   */
  async resources(): Promise<Resources> {
    const response = await this.sendCommand({ type: 'GetResources' });
    if (response.type === 'Success') {
      return response.payload;
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Run a script or command.
   */
  async run(
    script: string,
    options: {
      args?: string[];
      ram?: string;
      cpu?: number;
      node?: string;
    } = {}
  ): Promise<string> {
    const command: Record<string, unknown> = {
      type: 'RunJob',
      name: script,
      job_type: 'custom',
      args: options.args || [],
    };

    if (options.ram) {
      command.ram = this.parseRam(options.ram);
    }
    if (options.cpu) {
      command.cpu = options.cpu;
    }
    if (options.node) {
      command.node = options.node;
    }

    const response = await this.sendCommand(command);
    if (response.type === 'Success') {
      return response.payload.job_id;
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Get job status.
   */
  async jobStatus(jobId: string): Promise<JobStatus> {
    const response = await this.sendCommand({
      type: 'GetJobStatus',
      job_id: jobId,
    });
    if (response.type === 'Success') {
      return response.payload;
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * List all jobs.
   */
  async listJobs(): Promise<JobStatus[]> {
    const response = await this.sendCommand({ type: 'ListJobs' });
    if (response.type === 'Success') {
      return response.payload.jobs || [];
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Cancel a job.
   */
  async cancelJob(jobId: string): Promise<boolean> {
    const response = await this.sendCommand({
      type: 'CancelJob',
      job_id: jobId,
    });
    return response.type === 'Success';
  }

  /**
   * Create a memory pool.
   */
  async createMemoryPool(name: string, sizeGb: number): Promise<MemoryPoolInfo> {
    const response = await this.sendCommand({
      type: 'CreateMemoryPool',
      name,
      size_gb: sizeGb,
    });
    if (response.type === 'Success') {
      return response.payload;
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * List memory pools.
   */
  async listMemoryPools(): Promise<MemoryPoolInfo[]> {
    const response = await this.sendCommand({ type: 'ListMemoryPools' });
    if (response.type === 'Success') {
      return response.payload.pools || [];
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Read a remote memory page from another node.
   */
  async readRemoteMemoryPage(poolId: string, pageId: number, targetNode: string): Promise<number[]> {
    const response = await this.sendCommand({
      type: 'ReadRemoteMemoryPage',
      pool_id: poolId,
      page_id: pageId,
      target_node: targetNode,
    });
    if (response.type === 'Success') {
      // Decode base64 to bytes
      const b64 = response.payload.data;
      const bytes = Buffer.from(b64, 'base64');
      return Array.from(bytes);
    }
    throw new Error(response.message || 'Unknown error');
  }

  /**
   * Write a remote memory page to another node.
   */
  async writeRemoteMemoryPage(poolId: string, pageId: number, data: number[], targetNode: string): Promise<boolean> {
    // Encode bytes to base64
    const buf = Buffer.from(data);
    const b64 = buf.toString('base64');
    const response = await this.sendCommand({
      type: 'WriteRemoteMemoryPage',
      pool_id: poolId,
      page_id: pageId,
      data: b64,
      target_node: targetNode,
    });
    return response.type === 'Success';
  }

  private parseRam(ram: string): number {
    const upper = ram.toUpperCase().trim();
    if (upper.endsWith('GB')) {
      return Math.floor(parseFloat(upper.slice(0, -2)) * 1024 * 1024 * 1024);
    } else if (upper.endsWith('MB')) {
      return Math.floor(parseFloat(upper.slice(0, -2)) * 1024 * 1024);
    } else if (upper.endsWith('KB')) {
      return Math.floor(parseFloat(upper.slice(0, -2)) * 1024);
    }
    return parseInt(upper, 10);
  }
}
