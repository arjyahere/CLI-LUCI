use anyhow::Result;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferMetadata {
    pub id: String,
    pub size_bytes: u64,
    pub page_size: u32,
    pub total_pages: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct BufferPage {
    pub page_id: u64,
    pub data: Bytes,
    pub owner_node: String,
    pub is_dirty: bool,
    pub access_count: u64,
}

pub struct DistributedBuffer {
    metadata: BufferMetadata,
    pages: Arc<RwLock<HashMap<u64, BufferPage>>>,
    local_node_id: String,
}

impl DistributedBuffer {
    pub fn new(id: &str, size_bytes: u64, local_node_id: &str) -> Self {
        let page_size = 4096u32;
        let total_pages = size_bytes.div_ceil(page_size as u64);

        Self {
            metadata: BufferMetadata {
                id: id.to_string(),
                size_bytes,
                page_size,
                total_pages,
                created_at: chrono::Utc::now(),
            },
            pages: Arc::new(RwLock::new(HashMap::new())),
            local_node_id: local_node_id.to_string(),
        }
    }

    pub fn metadata(&self) -> &BufferMetadata {
        &self.metadata
    }

    pub async fn read_page(&self, page_id: u64) -> Result<Bytes> {
        let pages = self.pages.read().await;

        if let Some(page) = pages.get(&page_id) {
            return Ok(page.data.clone());
        }

        drop(pages);

        tracing::warn!(
            "Page {} not cached locally on node {} - remote fetch not yet implemented",
            page_id,
            self.local_node_id
        );

        Err(anyhow::anyhow!(
            "Page {} not cached locally (remote fetch not yet implemented for buffer {})",
            page_id,
            self.metadata.id
        ))
    }

    pub async fn write_page(&self, page_id: u64, data: Bytes) -> Result<()> {
        let mut pages = self.pages.write().await;

        let page = BufferPage {
            page_id,
            data,
            owner_node: self.local_node_id.clone(),
            is_dirty: true,
            access_count: 0,
        };

        pages.insert(page_id, page);
        Ok(())
    }

    pub async fn read(&self, offset: u64, len: u64) -> Result<Bytes> {
        let page_size = self.metadata.page_size as u64;
        let start_page = offset / page_size;
        let end_page = (offset + len - 1) / page_size;

        let mut result = Vec::new();

        for page_id in start_page..=end_page {
            let page_data = self.read_page(page_id).await?;

            let page_start = if page_id == start_page {
                (offset % page_size) as usize
            } else {
                0
            };

            let page_end = if page_id == end_page {
                ((offset + len) % page_size) as usize
            } else {
                page_data.len()
            };

            result.extend_from_slice(&page_data[page_start..page_end]);
        }

        Ok(Bytes::from(result))
    }

    pub async fn write(&self, offset: u64, data: &[u8]) -> Result<()> {
        let page_size = self.metadata.page_size as u64;
        let start_page = offset / page_size;
        let end_page = (offset + data.len() as u64 - 1) / page_size;

        for page_id in start_page..=end_page {
            let page_start = if page_id == start_page {
                (offset % page_size) as usize
            } else {
                0
            };

            let page_end = if page_id == end_page {
                ((offset + data.len() as u64) % page_size) as usize
            } else {
                self.metadata.page_size as usize
            };

            let page_data = Bytes::copy_from_slice(&data[page_start..page_end]);
            self.write_page(page_id, page_data).await?;
        }

        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        let mut pages = self.pages.write().await;
        let mut flushed_count = 0u64;

        for page in pages.values_mut() {
            if page.is_dirty {
                if page.owner_node == self.local_node_id {
                    page.is_dirty = false;
                    flushed_count += 1;
                } else {
                    tracing::debug!(
                        "Skipping flush for page {} owned by remote node {}",
                        page.page_id,
                        page.owner_node
                    );
                }
            }
        }

        tracing::debug!("Flushed {} dirty pages on node {}", flushed_count, self.local_node_id);
        Ok(())
    }

    pub async fn receive_page(&self, page_id: u64, data: Bytes, owner_node: &str) -> Result<()> {
        let mut pages = self.pages.write().await;
        let page = BufferPage {
            page_id,
            data,
            owner_node: owner_node.to_string(),
            is_dirty: false,
            access_count: 0,
        };
        pages.insert(page_id, page);
        Ok(())
    }

    pub async fn stats(&self) -> BufferStats {
        let pages = self.pages.read().await;

        let cached_pages = pages.len() as u64;
        let dirty_pages = pages.values().filter(|p| p.is_dirty).count() as u64;
        let total_access: u64 = pages.values().map(|p| p.access_count).sum();

        BufferStats {
            total_pages: self.metadata.total_pages,
            cached_pages,
            dirty_pages,
            total_access,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStats {
    pub total_pages: u64,
    pub cached_pages: u64,
    pub dirty_pages: u64,
    pub total_access: u64,
}
