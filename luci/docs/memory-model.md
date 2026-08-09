# Luci Memory Model

## Overview

Luci provides virtual memory pools that can span multiple machines. Memory is divided into fixed-size pages distributed across nodes.

---

## Current State (v0.1.0)

Memory pools are **metadata-only** — the daemon tracks pool information but does not yet distribute actual pages across nodes.

### What Works

```
luci memory create ai-pool 4     # Create 4GB pool (validated: 0-1024 GB)
luci memory ls                    # List pools
luci memory info <id>             # Show pool details
luci memory rm <id>               # Delete pool
```

### What's Missing

- Actual page storage and distribution
- Page migration between nodes
- Remote page fetch on cache miss
- Page-level read/write operations

---

## Memory Pool Structure

```rust
struct MemoryPool {
    id: String,                    // UUID
    name: String,                  // Human-readable name
    total_pages: u64,             // Number of pages
    page_size_bytes: u32,         // Always 4096 (4KB)
    owner_nodes: Vec<NodeId>,     // Nodes that store pages
    created_at: DateTime<Utc>,
}
```

### Example

```
Pool "ai-pool" (4 GB)
  total_pages = 4GB / 4096 = 1,048,576 pages
  page_size_bytes = 4096
  owner_nodes = ["node-a-uuid"]
```

---

## Page Model (Planned)

Each page is a 4KB chunk of data:

```rust
struct MemoryPage {
    page_id: u64,           // Sequential ID (0, 1, 2, ...)
    data: Bytes,            // 4KB of data
    owner_node: String,     // Which node stores this page
    is_dirty: bool,         // Modified since last sync
    access_count: u64,      // For LRU eviction
}
```

### Page States

```
Clean   → Up to date with owner, can be evicted
Dirty   → Modified locally, needs to be synced
Cached  → Copy from remote node, read-only
Migrating → Being moved between nodes
Evicted → Removed from local cache
```

---

## DistributedBuffer (Built, Not Wired)

The `luci-runtime` crate contains a `DistributedBuffer` that implements page-based read/write:

```rust
let buffer = DistributedBuffer::new("pool-id", 4 * 1024 * 1024 * 1024, "node-id");

// Write data at offset
buffer.write(0, b"hello world").await?;

// Read data from offset
let data = buffer.read(0, 11).await?;

// Flush dirty pages to owner
buffer.flush().await?;

// Get stats
let stats = buffer.stats().await;
// BufferStats { total_pages, cached_pages, dirty_pages, total_access }
```

### Page Operations

| Operation | Description | Status |
|-----------|-------------|--------|
| `read_page(page_id)` | Read a single page | Built (local only) |
| `write_page(page_id, data)` | Write a single page | Built |
| `read(offset, len)` | Read arbitrary range | Built |
| `write(offset, data)` | Write arbitrary range | Built |
| `flush()` | Sync dirty pages to owner | Built (local only) |
| `receive_page(page_id, data, owner)` | Store remote page locally | Built |
| `stats()` | Get buffer statistics | Built |

### What's Missing

- **Remote page fetch**: When reading a page not in local cache, fetch from owner node
- **Page migration**: Move pages between nodes for load balancing
- **Eviction policy**: LRU or LFU when cache is full
- **Write-back**: Async write of dirty pages to owner

---

## DistributedArray (Built, Not Wired)

The Rust SDK contains a `DistributedArray` that tracks chunk ownership:

```rust
let arr = DistributedArray::new(vec![1000], "f64");
arr.set(42, 3.14)?;
let val = arr.get(42)?;

// Distributed across multiple nodes
let arr = DistributedArray::distributed(vec![1000], "f64", &["node-a", "node-b"]);
```

### What's Missing

- Remote chunk access (get/set on non-local chunks)
- Chunk migration between nodes

---

## Planned: Full Memory Distribution

### Write Flow

```
Client writes to offset 1000:
    │
    ├── Calculate page_id = 1000 / 4096 = 0
    │
    ├── Check if page 0 is in local cache
    │   ├── Yes → Write to local cache, mark dirty
    │   └── No  → Fetch page 0 from owner node
    │            Write to local cache, mark dirty
    │
    └── Background: Flush dirty pages to owner
```

### Read Flow

```
Client reads from offset 1000:
    │
    ├── Calculate page_id = 1000 / 4096 = 0
    │
    ├── Check if page 0 is in local cache
    │   ├── Yes → Return data from cache
    │   └── No  → Fetch page 0 from owner node
    │            Store in local cache
    │            Return data
    │
    └── Update access_count for LRU
```

### Page Migration

```
Node A is overloaded:
    │
    ├── Scheduler picks Node B for some pages
    │
    ├── Node A sends MemoryPageWrite to Node B
    │   (page_id, data, compressed, encrypted)
    │
    ├── Node B stores page, sends MemoryPageAck
    │
    ├── Node A updates page owner to Node B
    │
    └── Node A evicts local copy
```

---

## Memory Pool Commands

### Create

```bash
luci memory create <name> <size_gb>
```

**Internal:**
1. Validate size_gb (must be > 0 and <= 1024)
2. Calculate `total_pages = size_gb * 1024^3 / 4096`
3. Generate pool UUID
4. Set `owner_nodes = [self]`
5. Store in `DaemonState.memory_pools`

### List

```bash
luci memory ls
```

**Internal:**
1. Read all pools from `DaemonState`
2. Format as table: ID, Name, Size

### Info

```bash
luci memory info <pool_id>
```

**Internal:**
1. Look up pool by ID
2. Display: name, pages, page_size, total_size, owner_nodes, created_at

### Delete

```bash
luci memory rm <pool_id>
```

**Internal:**
1. Remove from `DaemonState.memory_pools`
2. (Future: Evict all cached pages, notify owner nodes)

---

## Size Calculation

| Size | Pages | Example |
|------|-------|---------|
| 1 GB | 262,144 | Small ML model |
| 4 GB | 1,048,576 | Medium dataset |
| 16 GB | 4,194,304 | Large dataset |
| 64 GB | 16,777,216 | Full GPU VRAM |

---

## Performance Considerations

- **Page size**: 4KB balances granularity vs overhead
- **Local cache**: Hot pages stay in memory, avoid network round-trips
- **Dirty tracking**: Only sync modified pages
- **Compression**: Optional zstd compression for network transfer
- **Encryption**: Optional AES-256 for sensitive data

---

## API Reference

### IPC Commands

```rust
IpcCommand::CreateMemoryPool { name: String, size_gb: f64 }
IpcCommand::ListMemoryPools
IpcCommand::GetMemoryPoolInfo { pool_id: String }
IpcCommand::DeleteMemoryPool { pool_id: String }
```

### Protocol Messages (Future)

```rust
Payload::MemoryPageRequest { page_id: u64, pool_id: String }
Payload::MemoryPageResponse { page_id, pool_id, data, compressed, encrypted }
Payload::MemoryPageWrite { page_id, pool_id, data, compressed, encrypted }
Payload::MemoryPageAck { page_id, pool_id, success: bool }
```
