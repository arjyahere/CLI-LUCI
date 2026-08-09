use anyhow::Result;
use crate::LuciClient;

/// MemoryPool - Represents a distributed memory pool.
#[derive(Debug, Clone)]
pub struct MemoryPool {
    client: LuciClient,
    pub id: String,
    pub name: String,
    pub size_gb: f64,
}

impl MemoryPool {
    /// Create a new memory pool.
    pub fn create(client: LuciClient, name: &str, size_gb: f64) -> Result<Self> {
        let result = client.create_memory_pool(name, size_gb)?;
        Ok(Self {
            client,
            id: result["pool_id"].as_str().unwrap_or("").to_string(),
            name: name.to_string(),
            size_gb: result["total_size_gb"].as_f64().unwrap_or(size_gb),
        })
    }

    /// Delete the memory pool.
    pub fn delete(&self) -> Result<()> {
        let response = self.client.send_command(luci_common::IpcCommand::DeleteMemoryPool {
            pool_id: self.id.clone(),
        })?;
        match response {
            luci_common::IpcResponse::Success { .. } => Ok(()),
            luci_common::IpcResponse::Error { message } => Err(anyhow::anyhow!(message)),
        }
    }
}

impl std::fmt::Display for MemoryPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MemoryPool(id={}, name={}, size_gb={})",
            self.id, self.name, self.size_gb
        )
    }
}

/// A chunk of a distributed array stored on a specific node.
#[derive(Debug, Clone)]
struct ArrayChunk {
    start_index: usize,
    end_index: usize,
    owner_node: String,
    data: Vec<f64>,
}

/// DistributedArray - A distributed array spanning multiple devices.
/// Data is split into chunks, each potentially owned by a different node.
/// When local data is accessed, it's served directly.
/// When remote data is accessed, it triggers a network fetch via IPC.
#[derive(Debug, Clone)]
pub struct DistributedArray {
    client: Option<LuciClient>,
    pub shape: Vec<usize>,
    pub dtype: String,
    chunks: Vec<ArrayChunk>,
    total_len: usize,
}

impl DistributedArray {
    /// Create a new distributed array (zero-filled) on a single node.
    pub fn new(shape: Vec<usize>, dtype: &str) -> Self {
        let total_len: usize = shape.iter().product();
        let chunk = ArrayChunk {
            start_index: 0,
            end_index: total_len,
            owner_node: "local".to_string(),
            data: vec![0.0; total_len],
        };
        Self {
            client: None,
            shape,
            dtype: dtype.to_string(),
            chunks: vec![chunk],
            total_len,
        }
    }

    /// Create a new distributed array with a client for remote access.
    pub fn with_client(shape: Vec<usize>, dtype: &str, client: LuciClient) -> Self {
        let total_len: usize = shape.iter().product();
        let chunk = ArrayChunk {
            start_index: 0,
            end_index: total_len,
            owner_node: "local".to_string(),
            data: vec![0.0; total_len],
        };
        Self {
            client: Some(client),
            shape,
            dtype: dtype.to_string(),
            chunks: vec![chunk],
            total_len,
        }
    }

    /// Create a zero-filled array.
    pub fn zeros(shape: Vec<usize>, dtype: &str) -> Self {
        Self::new(shape, dtype)
    }

    /// Create a one-filled array.
    pub fn ones(shape: Vec<usize>, dtype: &str) -> Self {
        let total_len: usize = shape.iter().product();
        let chunk = ArrayChunk {
            start_index: 0,
            end_index: total_len,
            owner_node: "local".to_string(),
            data: vec![1.0; total_len],
        };
        Self {
            client: None,
            shape,
            dtype: dtype.to_string(),
            chunks: vec![chunk],
            total_len,
        }
    }

    /// Create a distributed array split across multiple nodes.
    pub fn distributed(shape: Vec<usize>, dtype: &str, node_ids: &[String]) -> Self {
        let total_len: usize = shape.iter().product();
        let chunk_size = total_len.div_ceil(node_ids.len());
        let mut chunks = Vec::new();

        for (i, node_id) in node_ids.iter().enumerate() {
            let start = i * chunk_size;
            let end = std::cmp::min(start + chunk_size, total_len);
            if start < total_len {
                chunks.push(ArrayChunk {
                    start_index: start,
                    end_index: end,
                    owner_node: node_id.clone(),
                    data: vec![0.0; end - start],
                });
            }
        }

        Self {
            client: None,
            shape,
            dtype: dtype.to_string(),
            chunks,
            total_len,
        }
    }

    /// Create a distributed array with a client for remote access.
    pub fn distributed_with_client(shape: Vec<usize>, dtype: &str, node_ids: &[String], client: LuciClient) -> Self {
        let total_len: usize = shape.iter().product();
        let chunk_size = total_len.div_ceil(node_ids.len());
        let mut chunks = Vec::new();

        for (i, node_id) in node_ids.iter().enumerate() {
            let start = i * chunk_size;
            let end = std::cmp::min(start + chunk_size, total_len);
            if start < total_len {
                chunks.push(ArrayChunk {
                    start_index: start,
                    end_index: end,
                    owner_node: node_id.clone(),
                    data: vec![0.0; end - start],
                });
            }
        }

        Self {
            client: Some(client),
            shape,
            dtype: dtype.to_string(),
            chunks,
            total_len,
        }
    }

    /// Get the total number of elements.
    pub fn len(&self) -> usize {
        self.total_len
    }

    /// Check if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    /// Find which chunk contains the given flat index.
    fn find_chunk(&self, index: usize) -> Option<(usize, usize)> {
        for (i, chunk) in self.chunks.iter().enumerate() {
            if index >= chunk.start_index && index < chunk.end_index {
                return Some((i, index - chunk.start_index));
            }
        }
        None
    }

    /// Get a value at the flat index.
    pub fn get(&self, index: usize) -> Option<f64> {
        let (chunk_idx, local_idx) = self.find_chunk(index)?;
        let chunk = &self.chunks[chunk_idx];
        if chunk.owner_node == "local" {
            return chunk.data.get(local_idx).copied();
        }
        
        // Remote chunk - try to fetch via IPC
        if let Some(ref client) = self.client {
            let page_id = (index / 4096) as u64;
            let pool_id = format!("array_{}", self.dtype);
            match client.read_remote_memory_page(&pool_id, page_id, &chunk.owner_node) {
                Ok(data) => {
                    // Parse value from page data (8 bytes per f64)
                    let offset = (index % 4096) * 8;
                    if offset + 8 <= data.len() {
                        let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;
                        Some(f64::from_le_bytes(bytes))
                    } else {
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch remote chunk: {}", e);
                    None
                }
            }
        } else {
            tracing::warn!(
                "Accessing remote chunk at index {} (owner: {}) - no client configured",
                index,
                chunk.owner_node
            );
            None
        }
    }

    /// Set a value at the flat index.
    pub fn set(&mut self, index: usize, value: f64) -> Result<()> {
        let (chunk_idx, local_idx) = self.find_chunk(index)
            .ok_or_else(|| anyhow::anyhow!(
                "Index {} out of bounds for array of length {}",
                index,
                self.total_len
            ))?;
        let chunk = &mut self.chunks[chunk_idx];
        if chunk.owner_node == "local" {
            chunk.data[local_idx] = value;
            return Ok(());
        }
        
        // Remote chunk - try to write via IPC
        if let Some(ref client) = self.client {
            let page_id = (index / 4096) as u64;
            let pool_id = format!("array_{}", self.dtype);
            let offset = (index % 4096) * 8;
            let mut data = vec![0u8; 4096];
            let bytes = value.to_le_bytes();
            data[offset..offset + 8].copy_from_slice(&bytes);
            client.write_remote_memory_page(&pool_id, page_id, data, &chunk.owner_node)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Cannot set value at index {} - data owned by remote node {} and no client configured",
                index,
                chunk.owner_node
            ))
        }
    }

    /// Get the underlying data as a slice (only works for single-chunk local arrays).
    /// Returns None for distributed (multi-chunk) arrays.
    pub fn as_slice(&self) -> Option<&[f64]> {
        if self.chunks.len() == 1 && self.chunks[0].owner_node == "local" {
            Some(&self.chunks[0].data)
        } else {
            None
        }
    }

    /// Get the number of chunks.
    pub fn num_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Get the owner node for a given index.
    pub fn owner_at(&self, index: usize) -> Option<&str> {
        let (chunk_idx, _) = self.find_chunk(index)?;
        Some(&self.chunks[chunk_idx].owner_node)
    }

    /// Get the total number of local elements.
    pub fn local_len(&self) -> usize {
        self.chunks.iter()
            .filter(|c| c.owner_node == "local")
            .map(|c| c.end_index - c.start_index)
            .sum()
    }
}

impl std::fmt::Display for DistributedArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DistributedArray(shape={:?}, dtype={}, len={}, chunks={}, local={})",
            self.shape,
            self.dtype,
            self.total_len,
            self.chunks.len(),
            self.local_len()
        )
    }
}
