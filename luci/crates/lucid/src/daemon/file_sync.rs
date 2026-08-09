use anyhow::{Context, Result};
use luci_common::NodeId;
use luci_protocol::FileManifestEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks

/// Track active file transfers
pub struct FileTransferManager {
    /// Active transfers by transfer_id
    active_transfers: Arc<RwLock<HashMap<String, TransferState>>>,
    /// Node ID for this daemon
    node_id: NodeId,
}

struct TransferState {
    _transfer_id: String,
    _root_path: PathBuf,
    total_files: u32,
    total_bytes: u64,
    files_sent: u32,
    bytes_sent: u64,
    started_at: chrono::DateTime<chrono::Utc>,
}

impl TransferState {
    fn progress(&self) -> f64 {
        if self.total_files == 0 {
            return 100.0;
        }
        (self.files_sent as f64 / self.total_files as f64) * 100.0
    }

    fn elapsed_secs(&self) -> f64 {
        (chrono::Utc::now() - self.started_at).num_milliseconds() as f64 / 1000.0
    }
}

impl FileTransferManager {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            active_transfers: Arc::new(RwLock::new(HashMap::new())),
            node_id,
        }
    }

    /// Build a file manifest for a directory
    pub async fn build_manifest(root: &Path) -> Result<Vec<FileManifestEntry>> {
        let root = root.to_path_buf();
        let mut files = Vec::new();
        
        tokio::task::spawn_blocking(move || {
            Self::walk_directory(&root, &root, &mut files)?;
            Ok(files)
        }).await?
    }

    fn walk_directory(root: &Path, current: &Path, files: &mut Vec<FileManifestEntry>) -> Result<()> {
        if !current.exists() {
            return Ok(());
        }

        if current.is_dir() {
            for entry in std::fs::read_dir(current)? {
                let entry = entry?;
                let path = entry.path();
                
                // Skip hidden files, __pycache__, .git, node_modules
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy();
                    if name.starts_with('.') || name == "__pycache__" || name == "node_modules" {
                        continue;
                    }
                }

                if path.is_dir() {
                    Self::walk_directory(root, &path, files)?;
                } else if path.is_file() {
                    let content = std::fs::read(&path)?;
                    let hash = blake3::hash(&content);
                    let metadata = std::fs::metadata(&path)?;
                    
                    let relative_path = path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    files.push(FileManifestEntry {
                        relative_path,
                        size: metadata.len(),
                        blake3_hash: hex::encode(hash.as_bytes()),
                        modified_at: chrono::DateTime::from(metadata.modified().unwrap_or(std::time::SystemTime::now())),
                    });
                }
            }
        }

        Ok(())
    }

    /// Compare two manifests and return files that need to be synced
    pub fn diff_manifests(
        local: &[FileManifestEntry],
        remote: &[FileManifestEntry],
    ) -> Vec<String> {
        let remote_map: HashMap<&str, &FileManifestEntry> = remote.iter()
            .map(|e| (e.relative_path.as_str(), e))
            .collect();

        let mut needed = Vec::new();
        
        for entry in local {
            match remote_map.get(entry.relative_path.as_str()) {
                Some(remote_entry) => {
                    // File exists remotely, check if hash matches
                    if remote_entry.blake3_hash != entry.blake3_hash {
                        needed.push(entry.relative_path.clone());
                    }
                }
                None => {
                    // File doesn't exist remotely
                    needed.push(entry.relative_path.clone());
                }
            }
        }

        needed
    }

    /// Send files to a remote node
    pub async fn send_files(
        &self,
        local_root: &Path,
        target_node: &str,
        network_manager: &crate::daemon::network::NetworkManager,
        needed_files: &[String],
    ) -> Result<u64> {
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let total_files = needed_files.len() as u32;
        
        // Build manifest of needed files
        let manifest: Vec<FileManifestEntry> = needed_files.iter()
            .filter_map(|path| {
                let full_path = local_root.join(path);
                let content = std::fs::read(&full_path).ok()?;
                let hash = blake3::hash(&content);
                let metadata = std::fs::metadata(&full_path).ok()?;
                
                Some(FileManifestEntry {
                    relative_path: path.clone(),
                    size: metadata.len(),
                    blake3_hash: hex::encode(hash.as_bytes()),
                    modified_at: chrono::DateTime::from(metadata.modified().unwrap_or(std::time::SystemTime::now())),
                })
            })
            .collect();

        let total_bytes: u64 = manifest.iter().map(|e| e.size).sum();

        // Send manifest to target node
        let manifest_msg = luci_protocol::file_manifest_message(
            &self.node_id.to_string(),
            &transfer_id,
            &local_root.to_string_lossy(),
            manifest,
        );

        // Get peer address and send
        let peer_addr = format!("{}:{}", target_node, 9528); // Default node port
        network_manager.send_message_to_peer(&peer_addr, &manifest_msg).await?;

        // Track transfer
        {
            let mut transfers = self.active_transfers.write().await;
            transfers.insert(transfer_id.clone(), TransferState {
                _transfer_id: transfer_id.clone(),
                _root_path: local_root.to_path_buf(),
                total_files,
                total_bytes,
                files_sent: 0,
                bytes_sent: 0,
                started_at: chrono::Utc::now(),
            });
        }

        // Send each file in chunks
        let mut total_sent = 0u64;
        for (file_idx, file_path) in needed_files.iter().enumerate() {
            let full_path = local_root.join(file_path);
            let bytes_sent = self.send_file_chunked(
                &transfer_id,
                &full_path,
                file_path,
                network_manager,
                &peer_addr,
            ).await?;
            total_sent += bytes_sent;

            // Update progress
            {
                let mut transfers = self.active_transfers.write().await;
                if let Some(state) = transfers.get_mut(&transfer_id) {
                    state.files_sent = (file_idx + 1) as u32;
                    state.bytes_sent = total_sent;
                }
            }

            // Log progress
            {
                let transfers = self.active_transfers.read().await;
                if let Some(state) = transfers.get(&transfer_id) {
                    tracing::debug!(
                        "File sync progress: {:.1}% ({}/{} files, {:.2}s elapsed)",
                        state.progress(),
                        state.files_sent,
                        state.total_files,
                        state.elapsed_secs()
                    );
                }
            }
        }

        // Send completion message
        let complete_msg = luci_protocol::file_transfer_complete_message(
            &self.node_id.to_string(),
            &transfer_id,
            total_files,
            total_bytes,
        );
        network_manager.send_message_to_peer(&peer_addr, &complete_msg).await?;

        // Remove from active transfers
        {
            let mut transfers = self.active_transfers.write().await;
            transfers.remove(&transfer_id);
        }

        tracing::info!(
            "File sync complete: {} files ({} bytes) to {}",
            total_files,
            total_sent,
            target_node
        );

        Ok(total_sent)
    }

    /// Send a single file in chunks
    async fn send_file_chunked(
        &self,
        transfer_id: &str,
        file_path: &Path,
        relative_path: &str,
        network_manager: &crate::daemon::network::NetworkManager,
        peer_addr: &str,
    ) -> Result<u64> {
        let content = tokio::fs::read(file_path).await
            .context(format!("Failed to read file: {:?}", file_path))?;
        
        let total_chunks = content.len().div_ceil(CHUNK_SIZE) as u32;
        let file_size = content.len() as u64;

        for (chunk_idx, chunk) in content.chunks(CHUNK_SIZE).enumerate() {
            let chunk_hash = blake3::hash(chunk);
            
            let chunk_msg = luci_protocol::file_chunk_message(
                &self.node_id.to_string(),
                transfer_id,
                relative_path,
                chunk_idx as u32,
                total_chunks,
                chunk.to_vec(),
                &hex::encode(chunk_hash.as_bytes()),
            );

            network_manager.send_message_to_peer(peer_addr, &chunk_msg).await?;
        }

        Ok(file_size)
    }

    /// Handle incoming file manifest - returns files that need to be synced
    pub async fn handle_manifest(
        _transfer_id: &str,
        _root_path: &str,
        files: &[FileManifestEntry],
        local_root: &Path,
    ) -> Vec<String> {
        // Build local manifest
        let local_manifest = match Self::build_manifest(local_root).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Failed to build local manifest: {}", e);
                return Vec::new();
            }
        };

        // Find files we need
        Self::diff_manifests(files, &local_manifest)
    }

    /// Handle incoming file chunk
    #[allow(clippy::too_many_arguments)]
    pub async fn handle_chunk(
        &self,
        _transfer_id: &str,
        file_path: &str,
        chunk_index: u32,
        total_chunks: u32,
        data: &[u8],
        chunk_hash: &str,
        target_root: &Path,
    ) -> Result<()> {
        // Verify chunk hash
        let actual_hash = blake3::hash(data);
        if hex::encode(actual_hash.as_bytes()) != chunk_hash {
            anyhow::bail!("Chunk hash mismatch for {} chunk {}", file_path, chunk_index);
        }

        // Create target directory
        let full_path = target_root.join(file_path);
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Append chunk to file (or create new file)
        if chunk_index == 0 {
            // First chunk - create/overwrite file
            tokio::fs::write(&full_path, data).await?;
        } else {
            // Subsequent chunks - append
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(&full_path)
                .await?;
            file.write_all(data).await?;
        }

        tracing::debug!(
            "Received chunk {}/{} for {} ({} bytes)",
            chunk_index + 1,
            total_chunks,
            file_path,
            data.len()
        );

        Ok(())
    }

    /// Get transfer progress
    pub async fn get_progress(&self, transfer_id: &str) -> Option<(u32, u32, u64, u64)> {
        let transfers = self.active_transfers.read().await;
        transfers.get(transfer_id).map(|t| {
            (t.files_sent, t.total_files, t.bytes_sent, t.total_bytes)
        })
    }
}
