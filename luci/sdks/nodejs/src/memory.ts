import { LuciClient } from './client';

export interface MemoryPoolData {
  id: string;
  name: string;
  size_gb: number;
}

/**
 * MemoryPool - Represents a distributed memory pool.
 */
export class MemoryPool {
  private client: LuciClient;
  public id: string;
  public name: string;
  public sizeGb: number;

  constructor(client: LuciClient, id: string, name: string, sizeGb: number) {
    this.client = client;
    this.id = id;
    this.name = name;
    this.sizeGb = sizeGb;
  }

  /**
   * Create a new memory pool.
   */
  static async create(
    client: LuciClient,
    name: string,
    sizeGb: number
  ): Promise<MemoryPool> {
    const result = await client.createMemoryPool(name, sizeGb);
    return new MemoryPool(
      client,
      result.pool_id,
      name,
      result.total_size_gb
    );
  }

  /**
   * Delete the memory pool.
   */
  async delete(): Promise<boolean> {
    try {
      await this.client.sendCommand({
        type: 'DeleteMemoryPool',
        pool_id: this.id,
      });
      return true;
    } catch {
      return false;
    }
  }

  toString(): string {
    return `MemoryPool(id=${this.id}, name=${this.name}, sizeGb=${this.sizeGb})`;
  }
}

export interface ArrayChunk {
  startIndex: number;
  endIndex: number;
  ownerNode: string;
  data: number[];
}

/**
 * DistributedArray - A distributed array spanning multiple devices.
 * Data is split into chunks, each potentially owned by a different node.
 */
export class DistributedArray {
  public shape: number[];
  public dtype: string;
  private client: LuciClient | null;
  private chunks: ArrayChunk[];
  private totalLen: number;

  constructor(shape: number[], dtype: string = 'float32', client: LuciClient | null = null) {
    this.shape = shape;
    this.dtype = dtype;
    this.client = client;
    this.totalLen = shape.reduce((a, b) => a * b, 1);
    this.chunks = [{
      startIndex: 0,
      endIndex: this.totalLen,
      ownerNode: 'local',
      data: new Array(this.totalLen).fill(0),
    }];
  }

  /**
   * Create a zero-filled array.
   */
  static zeros(shape: number[], dtype: string = 'float32'): DistributedArray {
    return new DistributedArray(shape, dtype);
  }

  /**
   * Create a one-filled array.
   */
  static ones(shape: number[], dtype: string = 'float32'): DistributedArray {
    const arr = new DistributedArray(shape, dtype);
    arr.chunks[0].data = new Array(arr.totalLen).fill(1);
    return arr;
  }

  /**
   * Create a distributed array split across multiple nodes.
   */
  static distributed(shape: number[], dtype: string, nodeIds: string[]): DistributedArray {
    const arr = new DistributedArray(shape, dtype);
    const totalLen = arr.totalLen;
    const chunkSize = Math.ceil(totalLen / nodeIds.length);
    
    arr.chunks = nodeIds.map((nodeId, i) => {
      const start = i * chunkSize;
      const end = Math.min(start + chunkSize, totalLen);
      return {
        startIndex: start,
        endIndex: end,
        ownerNode: nodeId,
        data: new Array(end - start).fill(0),
      };
    }).filter(c => c.startIndex < totalLen);
    
    return arr;
  }

  private findChunk(index: number): [number, number] | null {
    for (let i = 0; i < this.chunks.length; i++) {
      const chunk = this.chunks[i];
      if (index >= chunk.startIndex && index < chunk.endIndex) {
        return [i, index - chunk.startIndex];
      }
    }
    return null;
  }

  /**
   * Get a value at the flat index.
   */
  async get(index: number): Promise<number | null> {
    const result = this.findChunk(index);
    if (!result) return null;
    
    const [chunkIdx, localIdx] = result;
    const chunk = this.chunks[chunkIdx];
    
    if (chunk.ownerNode === 'local') {
      return chunk.data[localIdx];
    }
    
    // Remote chunk - try to fetch via IPC
    if (this.client) {
      const pageIndex = Math.floor(index / 4096);
      const poolId = `array_${this.dtype}`;
      try {
        const data = await this.client.readRemoteMemoryPage(poolId, pageIndex, chunk.ownerNode);
        const offset = (index % 4096) * 8;
        if (offset + 8 <= data.length) {
          // Parse as float64 little-endian
          const buf = Buffer.from(data.slice(offset, offset + 8));
          return buf.readDoubleLE(0);
        }
      } catch (e) {
        console.warn(`Failed to fetch remote chunk: ${e}`);
      }
    }
    return null;
  }

  /**
   * Set a value at the flat index.
   */
  async set(index: number, value: number): Promise<boolean> {
    const result = this.findChunk(index);
    if (!result) return false;
    
    const [chunkIdx, localIdx] = result;
    const chunk = this.chunks[chunkIdx];
    
    if (chunk.ownerNode === 'local') {
      chunk.data[localIdx] = value;
      return true;
    }
    
    // Remote chunk - try to write via IPC
    if (this.client) {
      const pageIndex = Math.floor(index / 4096);
      const poolId = `array_${this.dtype}`;
      try {
        const offset = (index % 4096) * 8;
        const data = Buffer.alloc(4096);
        data.writeDoubleLE(value, offset);
        await this.client.writeRemoteMemoryPage(poolId, pageIndex, Array.from(data), chunk.ownerNode);
        return true;
      } catch (e) {
        console.warn(`Failed to write remote chunk: ${e}`);
      }
    }
    return false;
  }

  /**
   * Get the total number of elements.
   */
  get length(): number {
    return this.totalLen;
  }

  /**
   * Get the number of chunks.
   */
  get numChunks(): number {
    return this.chunks.length;
  }

  /**
   * Get the owner node for a given index.
   */
  ownerAt(index: number): string | null {
    const result = this.findChunk(index);
    if (!result) return null;
    return this.chunks[result[0]].ownerNode;
  }

  toString(): string {
    return `DistributedArray(shape=${JSON.stringify(this.shape)}, dtype=${this.dtype}, len=${this.totalLen}, chunks=${this.chunks.length})`;
  }
}
