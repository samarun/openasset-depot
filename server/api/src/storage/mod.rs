use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::{Duration, SystemTime},
};

use async_stream::try_stream;
use blake3::Hasher;
use bytes::Bytes;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    pub hash: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobManifest {
    pub hash: String,
    pub size_bytes: u64,
    pub chunks: Vec<ChunkRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobIntegrityReport {
    pub blob_hash: String,
    pub ok: bool,
    pub size_bytes: u64,
    pub chunk_count: usize,
    pub missing_chunks: Vec<String>,
    pub hash_mismatches: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LocalObjectStore {
    root: PathBuf,
    chunk_size: usize,
}

impl LocalObjectStore {
    pub async fn new(root: PathBuf, chunk_size: usize) -> AppResult<Self> {
        if chunk_size == 0 {
            return Err(AppError::configuration(
                "chunk size must be greater than zero",
            ));
        }
        let store = Self { root, chunk_size };
        fs::create_dir_all(store.chunks_root()).await?;
        fs::create_dir_all(store.blobs_root()).await?;
        fs::create_dir_all(store.tmp_root()).await?;
        store
            .cleanup_stale_uploads(Duration::from_secs(24 * 60 * 60))
            .await?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn chunk_files_root(&self) -> PathBuf {
        self.chunks_root()
    }

    pub fn blob_manifests_root(&self) -> PathBuf {
        self.blobs_root()
    }

    pub fn temp_upload_path(&self, upload_id: Uuid) -> PathBuf {
        self.tmp_root().join(format!("{upload_id}.upload"))
    }

    pub async fn put_file(&self, path: &Path) -> AppResult<BlobManifest> {
        let file = fs::File::open(path).await?;
        self.put_reader(file).await
    }

    pub async fn cleanup_stale_uploads(&self, max_age: Duration) -> AppResult<usize> {
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

    pub async fn put_reader<R>(&self, mut reader: R) -> AppResult<BlobManifest>
    where
        R: AsyncRead + Unpin,
    {
        let mut file_hasher = Hasher::new();
        let mut chunks = Vec::new();
        let mut total_size = 0u64;
        let mut buffer = vec![0u8; self.chunk_size];

        loop {
            let mut filled = 0usize;
            while filled < self.chunk_size {
                let read = reader.read(&mut buffer[filled..]).await?;
                if read == 0 {
                    break;
                }
                filled += read;
            }
            if filled == 0 {
                break;
            }

            let bytes = &buffer[..filled];
            file_hasher.update(bytes);
            total_size += filled as u64;
            let chunk_hash = blake3::hash(bytes).to_hex().to_string();
            self.write_chunk_if_missing(&chunk_hash, bytes).await?;
            chunks.push(ChunkRef {
                hash: chunk_hash,
                size_bytes: filled as u64,
            });
        }

        let hash = file_hasher.finalize().to_hex().to_string();
        let manifest = BlobManifest {
            hash: hash.clone(),
            size_bytes: total_size,
            chunks,
        };
        self.write_blob_manifest_if_missing(&manifest).await?;
        Ok(manifest)
    }

    pub async fn read_manifest(&self, blob_hash: &str) -> AppResult<BlobManifest> {
        let path = self.blob_path(blob_hash);
        let data = fs::read(path).await?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn stream_blob(
        &self,
        blob_hash: String,
    ) -> Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static>> {
        let store = self.clone();
        Box::pin(try_stream! {
            let manifest = store.read_manifest(&blob_hash).await.map_err(std::io::Error::other)?;
            let mut buffer = vec![0u8; 64 * 1024];
            for chunk in manifest.chunks {
                let mut file = fs::File::open(store.chunk_path(&chunk.hash)).await?;
                loop {
                    let read = file.read(&mut buffer).await?;
                    if read == 0 {
                        break;
                    }
                    yield Bytes::copy_from_slice(&buffer[..read]);
                }
            }
        })
    }

    pub async fn write_blob_to_path(&self, blob_hash: &str, target: &Path) -> AppResult<()> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await?;
        }
        let manifest = self.read_manifest(blob_hash).await?;
        let tmp_path = target.with_extension("oadtmp");
        let mut out = fs::File::create(&tmp_path).await?;
        let mut buffer = vec![0u8; 64 * 1024];
        for chunk in manifest.chunks {
            let mut file = fs::File::open(self.chunk_path(&chunk.hash)).await?;
            loop {
                let read = file.read(&mut buffer).await?;
                if read == 0 {
                    break;
                }
                out.write_all(&buffer[..read]).await?;
            }
        }
        out.flush().await?;
        fs::rename(tmp_path, target).await?;
        Ok(())
    }

    pub async fn verify_blob(&self, blob_hash: &str) -> BlobIntegrityReport {
        let manifest = match self.read_manifest(blob_hash).await {
            Ok(manifest) => manifest,
            Err(error) => {
                return BlobIntegrityReport {
                    blob_hash: blob_hash.to_string(),
                    ok: false,
                    size_bytes: 0,
                    chunk_count: 0,
                    missing_chunks: Vec::new(),
                    hash_mismatches: Vec::new(),
                    message: Some(format!("manifest read failed: {error}")),
                };
            }
        };

        let mut missing_chunks = Vec::new();
        let mut hash_mismatches = Vec::new();
        if manifest.hash != blob_hash {
            hash_mismatches.push("manifest_hash".to_string());
        }

        let mut blob_hasher = Hasher::new();
        let mut total_size = 0u64;
        let mut buffer = vec![0u8; 64 * 1024];
        for chunk in &manifest.chunks {
            let mut chunk_hasher = Hasher::new();
            let mut chunk_size = 0u64;
            let mut file = match fs::File::open(self.chunk_path(&chunk.hash)).await {
                Ok(file) => file,
                Err(_) => {
                    missing_chunks.push(chunk.hash.clone());
                    continue;
                }
            };
            loop {
                let read = match file.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(error) => {
                        hash_mismatches.push(format!("{}:read:{error}", chunk.hash));
                        break;
                    }
                };
                chunk_hasher.update(&buffer[..read]);
                blob_hasher.update(&buffer[..read]);
                chunk_size += read as u64;
                total_size += read as u64;
            }
            if chunk_hasher.finalize().to_hex().as_str() != chunk.hash {
                hash_mismatches.push(chunk.hash.clone());
            }
            if chunk_size != chunk.size_bytes {
                hash_mismatches.push(format!("{}:size", chunk.hash));
            }
        }

        let actual_blob_hash = blob_hasher.finalize().to_hex().to_string();
        if actual_blob_hash != manifest.hash {
            hash_mismatches.push("blob_hash".to_string());
        }
        if total_size != manifest.size_bytes {
            hash_mismatches.push("blob_size".to_string());
        }

        BlobIntegrityReport {
            blob_hash: blob_hash.to_string(),
            ok: missing_chunks.is_empty() && hash_mismatches.is_empty(),
            size_bytes: total_size,
            chunk_count: manifest.chunks.len(),
            missing_chunks,
            hash_mismatches,
            message: None,
        }
    }

    fn chunks_root(&self) -> PathBuf {
        self.root.join("chunks")
    }

    fn blobs_root(&self) -> PathBuf {
        self.root.join("blobs")
    }

    fn tmp_root(&self) -> PathBuf {
        self.root.join("uploads").join("tmp")
    }

    fn chunk_path(&self, hash: &str) -> PathBuf {
        shard_path(self.chunks_root(), hash)
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        shard_path(self.blobs_root(), &format!("{hash}.json"))
    }

    async fn write_chunk_if_missing(&self, hash: &str, bytes: &[u8]) -> AppResult<()> {
        let final_path = self.chunk_path(hash);
        if fs::try_exists(&final_path).await? {
            return Ok(());
        }
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp_path = self.tmp_root().join(format!("{}.chunk", Uuid::new_v4()));
        let mut file = fs::File::create(&tmp_path).await?;
        file.write_all(bytes).await?;
        file.flush().await?;
        match fs::rename(&tmp_path, &final_path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&tmp_path).await;
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn write_blob_manifest_if_missing(&self, manifest: &BlobManifest) -> AppResult<()> {
        let final_path = self.blob_path(&manifest.hash);
        if fs::try_exists(&final_path).await? {
            return Ok(());
        }
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp_path = self.tmp_root().join(format!("{}.blob", Uuid::new_v4()));
        let data = serde_json::to_vec_pretty(manifest)?;
        let mut file = fs::File::create(&tmp_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;
        match fs::rename(&tmp_path, &final_path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&tmp_path).await;
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }
}

fn shard_path(root: PathBuf, name: &str) -> PathBuf {
    let hash = name.trim_end_matches(".json");
    let first = &hash[0..2];
    let second = &hash[2..4];
    root.join(first).join(second).join(name)
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, time::Duration};

    use uuid::Uuid;

    use super::LocalObjectStore;

    #[tokio::test]
    async fn stores_and_restores_chunked_blob() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalObjectStore::new(dir.path().join("objects"), 4)
            .await
            .unwrap();
        let manifest = store
            .put_reader(Cursor::new(b"hello world".to_vec()))
            .await
            .unwrap();
        assert_eq!(manifest.size_bytes, 11);
        assert!(manifest.chunks.len() > 1);

        let target = dir.path().join("restored.bin");
        store
            .write_blob_to_path(&manifest.hash, &target)
            .await
            .unwrap();
        let restored = tokio::fs::read(target).await.unwrap();
        assert_eq!(restored, b"hello world");
    }

    #[tokio::test]
    async fn deduplicates_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalObjectStore::new(dir.path().join("objects"), 4)
            .await
            .unwrap();
        let first = store
            .put_reader(Cursor::new(b"same data".to_vec()))
            .await
            .unwrap();
        let second = store
            .put_reader(Cursor::new(b"same data".to_vec()))
            .await
            .unwrap();
        assert_eq!(first.hash, second.hash);
    }

    #[tokio::test]
    async fn streams_large_content_across_bounded_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let chunk_size = 128 * 1024;
        let store = LocalObjectStore::new(dir.path().join("objects"), chunk_size)
            .await
            .unwrap();
        let data = vec![42u8; chunk_size * 3 + 17];
        let manifest = store.put_reader(Cursor::new(data)).await.unwrap();

        assert_eq!(manifest.size_bytes, (chunk_size * 3 + 17) as u64);
        assert_eq!(manifest.chunks.len(), 4);
        assert_eq!(manifest.chunks.last().unwrap().size_bytes, 17);
    }

    #[tokio::test]
    async fn verifies_blob_integrity_and_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalObjectStore::new(dir.path().join("objects"), 4)
            .await
            .unwrap();
        let manifest = store
            .put_reader(Cursor::new(b"integrity".to_vec()))
            .await
            .unwrap();
        let clean = store.verify_blob(&manifest.hash).await;
        assert!(clean.ok);

        let first_chunk = manifest.chunks.first().unwrap();
        tokio::fs::write(store.chunk_path(&first_chunk.hash), b"bad")
            .await
            .unwrap();
        let corrupt = store.verify_blob(&manifest.hash).await;
        assert!(!corrupt.ok);
        assert!(!corrupt.hash_mismatches.is_empty());
    }

    #[tokio::test]
    async fn cleans_stale_upload_temporaries() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalObjectStore::new(dir.path().join("objects"), 4)
            .await
            .unwrap();
        let temporary = store.temp_upload_path(Uuid::new_v4());
        tokio::fs::write(&temporary, b"partial upload")
            .await
            .unwrap();

        let removed = store.cleanup_stale_uploads(Duration::ZERO).await.unwrap();
        assert_eq!(removed, 1);
        assert!(!temporary.exists());
    }
}
