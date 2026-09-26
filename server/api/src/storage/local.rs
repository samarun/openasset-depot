use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use tokio::{fs, io::AsyncWriteExt};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

use super::{
    backend::{ChunkBackend, ChunkReader},
    BlobManifest,
};

/// Filesystem-backed chunk storage.
///
/// Objects are sharded two levels deep by the first four hex characters of
/// their hash to keep directory sizes manageable on large depots. Writes land
/// in a temporary file and are then renamed, so a crash mid-write can never
/// publish a truncated object under a valid hash name.
#[derive(Debug, Clone)]
pub struct LocalChunkBackend {
    root: PathBuf,
}

impl LocalChunkBackend {
    pub async fn new(root: PathBuf) -> AppResult<Self> {
        let backend = Self { root };
        fs::create_dir_all(backend.chunks_root()).await?;
        fs::create_dir_all(backend.blobs_root()).await?;
        fs::create_dir_all(backend.tmp_root()).await?;
        Ok(backend)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn chunks_root(&self) -> PathBuf {
        self.root.join("chunks")
    }

    pub fn blobs_root(&self) -> PathBuf {
        self.root.join("blobs")
    }

    pub fn tmp_root(&self) -> PathBuf {
        self.root.join("uploads").join("tmp")
    }

    pub fn temp_upload_path(&self, upload_id: Uuid) -> PathBuf {
        self.tmp_root().join(format!("{upload_id}.upload"))
    }

    pub fn chunk_path(&self, hash: &str) -> PathBuf {
        shard_path(self.chunks_root(), hash)
    }

    pub fn blob_path(&self, hash: &str) -> PathBuf {
        shard_path(self.blobs_root(), &format!("{hash}.json"))
    }

    /// Writes `bytes` to `final_path` via a temporary file and an atomic rename.
    ///
    /// A concurrent writer publishing the identical object first is not an
    /// error: content-addressed objects are immutable, so both writers agree.
    async fn publish_atomically(
        &self,
        final_path: &Path,
        suffix: &str,
        bytes: &[u8],
    ) -> AppResult<()> {
        if fs::try_exists(final_path).await? {
            return Ok(());
        }
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp_path = self.tmp_root().join(format!("{}.{suffix}", Uuid::new_v4()));
        let mut file = fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.flush().await?;
        match fs::rename(&tmp_path, final_path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&tmp_path).await;
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }
}

#[async_trait]
impl ChunkBackend for LocalChunkBackend {
    async fn chunk_exists(&self, hash: &str) -> AppResult<bool> {
        Ok(fs::try_exists(self.chunk_path(hash)).await?)
    }

    async fn put_chunk(&self, hash: &str, bytes: &[u8]) -> AppResult<()> {
        let path = self.chunk_path(hash);
        self.publish_atomically(&path, "chunk", bytes).await
    }

    async fn open_chunk(&self, hash: &str) -> AppResult<ChunkReader> {
        let file = fs::File::open(self.chunk_path(hash)).await?;
        Ok(Box::pin(file))
    }

    async fn manifest_exists(&self, hash: &str) -> AppResult<bool> {
        Ok(fs::try_exists(self.blob_path(hash)).await?)
    }

    async fn put_manifest(&self, manifest: &BlobManifest) -> AppResult<()> {
        let path = self.blob_path(&manifest.hash);
        let data = serde_json::to_vec_pretty(manifest)?;
        self.publish_atomically(&path, "blob", &data).await
    }

    async fn read_manifest(&self, hash: &str) -> AppResult<BlobManifest> {
        let data = fs::read(self.blob_path(hash)).await?;
        Ok(serde_json::from_slice(&data)?)
    }

    async fn cleanup_stale_uploads(&self, max_age: Duration) -> AppResult<usize> {
        let cutoff = SystemTime::now()
            .checked_sub(max_age)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut removed = 0usize;
        let mut entries = fs::read_dir(self.tmp_root()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            if metadata.is_file() && metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH) <= cutoff
            {
                fs::remove_file(entry.path()).await?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn describe(&self) -> String {
        format!("local filesystem at {}", self.root.display())
    }
}

fn shard_path(root: PathBuf, name: &str) -> PathBuf {
    let hash = name.trim_end_matches(".json");
    let first = &hash[0..2];
    let second = &hash[2..4];
    root.join(first).join(second).join(name)
}

pub(super) fn validate_hash(hash: &str) -> AppResult<()> {
    if hash.len() < 4 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::bad_request(
            "object hash must be at least 4 hexadecimal characters",
        ));
    }
    Ok(())
}
