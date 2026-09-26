mod backend;
mod local;
#[cfg(feature = "s3")]
mod s3;

use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
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

pub use backend::{ChunkBackend, ChunkReader};
pub use local::LocalChunkBackend;
#[cfg(feature = "s3")]
pub use s3::S3ChunkBackend;

use crate::error::{AppError, AppResult};

/// Buffer used when copying chunk bytes; independent of the configured chunk size.
const COPY_BUFFER_BYTES: usize = 64 * 1024;

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

/// Content-addressed blob storage over a pluggable [`ChunkBackend`].
///
/// This type owns everything that must behave identically regardless of where
/// bytes live: fixed-size chunking, BLAKE3 hashing, deduplication, manifest
/// assembly, range reads, and integrity verification. The backend only sees
/// immutable objects named by hash.
#[derive(Debug, Clone)]
pub struct ObjectStore {
    backend: Arc<dyn ChunkBackend>,
    chunk_size: usize,
    /// Present only for the local backend, where callers still need real paths
    /// (staging uploads, admin tooling, tests).
    local: Option<LocalChunkBackend>,
}

/// Retained name for the default deployment, which stores objects on disk.
pub type LocalObjectStore = ObjectStore;

impl ObjectStore {
    /// Builds a store backed by the local filesystem.
    pub async fn new(root: PathBuf, chunk_size: usize) -> AppResult<Self> {
        let local = LocalChunkBackend::new(root).await?;
        let store = Self::with_backend(Arc::new(local.clone()), chunk_size)?;
        store
            .cleanup_stale_uploads(Duration::from_secs(24 * 60 * 60))
            .await?;
        Ok(Self {
            local: Some(local),
            ..store
        })
    }

    pub fn with_backend(backend: Arc<dyn ChunkBackend>, chunk_size: usize) -> AppResult<Self> {
        if chunk_size == 0 {
            return Err(AppError::configuration(
                "chunk size must be greater than zero",
            ));
        }
        Ok(Self {
            backend,
            chunk_size,
            local: None,
        })
    }

    pub fn describe(&self) -> String {
        self.backend.describe()
    }

    /// Filesystem root, when this store is backed by the local filesystem.
    pub fn local_root(&self) -> Option<&Path> {
        self.local.as_ref().map(LocalChunkBackend::root)
    }

    /// Panics for non-local backends; used by call sites that already require
    /// the default deployment.
    pub fn root(&self) -> &Path {
        self.local_root()
            .expect("root() is only available for the local object store backend")
    }

    pub fn chunk_files_root(&self) -> PathBuf {
        self.local
            .as_ref()
            .map(LocalChunkBackend::chunks_root)
            .unwrap_or_default()
    }

    pub fn blob_manifests_root(&self) -> PathBuf {
        self.local
            .as_ref()
            .map(LocalChunkBackend::blobs_root)
            .unwrap_or_default()
    }

    pub fn temp_upload_path(&self, upload_id: Uuid) -> PathBuf {
        self.local
            .as_ref()
            .map(|local| local.temp_upload_path(upload_id))
            .unwrap_or_else(|| std::env::temp_dir().join(format!("{upload_id}.upload")))
    }

    pub async fn cleanup_stale_uploads(&self, max_age: Duration) -> AppResult<usize> {
        self.backend.cleanup_stale_uploads(max_age).await
    }

    pub async fn put_file(&self, path: &Path) -> AppResult<BlobManifest> {
        let file = fs::File::open(path).await?;
        self.put_reader(file).await
    }

    /// Chunks `reader`, stores any chunks the backend does not already hold,
    /// and publishes the resulting manifest.
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
            if !self.backend.chunk_exists(&chunk_hash).await? {
                self.backend.put_chunk(&chunk_hash, bytes).await?;
            }
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
        if !self.backend.manifest_exists(&hash).await? {
            self.backend.put_manifest(&manifest).await?;
        }
        Ok(manifest)
    }

    pub async fn read_manifest(&self, blob_hash: &str) -> AppResult<BlobManifest> {
        local::validate_hash(blob_hash)?;
        self.backend.read_manifest(blob_hash).await
    }

    pub fn stream_blob(
        &self,
        blob_hash: String,
    ) -> Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static>> {
        let store = self.clone();
        Box::pin(try_stream! {
            let manifest = store.read_manifest(&blob_hash).await.map_err(std::io::Error::other)?;
            let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
            for chunk in manifest.chunks {
                let mut reader = store
                    .backend
                    .open_chunk(&chunk.hash)
                    .await
                    .map_err(std::io::Error::other)?;
                loop {
                    let read = reader.read(&mut buffer).await?;
                    if read == 0 {
                        break;
                    }
                    yield Bytes::copy_from_slice(&buffer[..read]);
                }
            }
        })
    }

    /// Streams the inclusive byte range `[start, end_inclusive]` of a blob.
    ///
    /// Chunks entirely before the range are skipped without being fetched,
    /// which is what makes ranged and resumed downloads cheap on any backend.
    pub fn stream_blob_range(
        &self,
        blob_hash: String,
        start: u64,
        end_inclusive: u64,
    ) -> Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static>> {
        let store = self.clone();
        Box::pin(try_stream! {
            let manifest = store.read_manifest(&blob_hash).await.map_err(std::io::Error::other)?;
            let mut chunk_start = 0u64;
            let mut remaining = end_inclusive.saturating_sub(start).saturating_add(1);
            let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
            for chunk in manifest.chunks {
                if remaining == 0 {
                    break;
                }
                let chunk_end = chunk_start + chunk.size_bytes;
                if chunk_end <= start {
                    chunk_start = chunk_end;
                    continue;
                }
                if chunk_start > end_inclusive {
                    break;
                }
                let offset = start.saturating_sub(chunk_start);
                let mut reader = store
                    .backend
                    .open_chunk(&chunk.hash)
                    .await
                    .map_err(std::io::Error::other)?;
                if offset > 0 {
                    skip_exact(&mut reader, offset).await?;
                }
                loop {
                    if remaining == 0 {
                        break;
                    }
                    let read_limit = buffer.len().min(remaining as usize);
                    let read = reader.read(&mut buffer[..read_limit]).await?;
                    if read == 0 {
                        break;
                    }
                    remaining -= read as u64;
                    yield Bytes::copy_from_slice(&buffer[..read]);
                }
                chunk_start = chunk_end;
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
        let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
        for chunk in manifest.chunks {
            let mut reader = self.backend.open_chunk(&chunk.hash).await?;
            loop {
                let read = reader.read(&mut buffer).await?;
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

    /// Re-reads and re-hashes every chunk of a blob.
    ///
    /// Never returns an error: a failed verification is a result, not an
    /// exception, so a scheduled sweep can report on a corrupt blob and carry
    /// on to the next one.
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
        let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
        for chunk in &manifest.chunks {
            let mut chunk_hasher = Hasher::new();
            let mut chunk_size = 0u64;
            let mut reader = match self.backend.open_chunk(&chunk.hash).await {
                Ok(reader) => reader,
                Err(_) => {
                    missing_chunks.push(chunk.hash.clone());
                    continue;
                }
            };
            loop {
                let read = match reader.read(&mut buffer).await {
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

    #[cfg(test)]
    fn chunk_path(&self, hash: &str) -> PathBuf {
        self.local
            .as_ref()
            .expect("chunk_path requires the local backend")
            .chunk_path(hash)
    }
}

/// Discards exactly `count` bytes from a reader that may not support seeking.
async fn skip_exact<R: AsyncRead + Unpin>(reader: &mut R, count: u64) -> std::io::Result<()> {
    let mut remaining = count;
    let mut scratch = vec![0u8; COPY_BUFFER_BYTES.min(count.max(1) as usize)];
    while remaining > 0 {
        let limit = scratch.len().min(remaining as usize);
        let read = reader.read(&mut scratch[..limit]).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "chunk ended before the requested range offset",
            ));
        }
        remaining -= read as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, time::Duration};

    use futures_core::Stream;
    use uuid::Uuid;

    use super::ObjectStore;

    async fn collect_stream(
        mut stream: std::pin::Pin<
            Box<dyn Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send>,
        >,
    ) -> Vec<u8> {
        use std::task::{Context, Poll};

        let mut out = Vec::new();
        std::future::poll_fn(|cx: &mut Context<'_>| loop {
            match stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => out.extend_from_slice(&bytes),
                Poll::Ready(Some(Err(error))) => panic!("stream failed: {error}"),
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => return Poll::Pending,
            }
        })
        .await;
        out
    }

    #[tokio::test]
    async fn stores_and_restores_chunked_blob() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path().join("objects"), 4)
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
        let store = ObjectStore::new(dir.path().join("objects"), 4)
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
        let store = ObjectStore::new(dir.path().join("objects"), chunk_size)
            .await
            .unwrap();
        let data = vec![42u8; chunk_size * 3 + 17];
        let manifest = store.put_reader(Cursor::new(data)).await.unwrap();

        assert_eq!(manifest.size_bytes, (chunk_size * 3 + 17) as u64);
        assert_eq!(manifest.chunks.len(), 4);
        assert_eq!(manifest.chunks.last().unwrap().size_bytes, 17);
    }

    #[tokio::test]
    async fn streams_a_byte_range_that_starts_mid_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path().join("objects"), 4)
            .await
            .unwrap();
        let manifest = store
            .put_reader(Cursor::new(b"abcdefghijkl".to_vec()))
            .await
            .unwrap();

        let ranged = collect_stream(store.stream_blob_range(manifest.hash.clone(), 5, 9)).await;
        assert_eq!(ranged, b"fghij");

        let whole = collect_stream(store.stream_blob(manifest.hash)).await;
        assert_eq!(whole, b"abcdefghijkl");
    }

    #[tokio::test]
    async fn verifies_blob_integrity_and_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path().join("objects"), 4)
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
        let store = ObjectStore::new(dir.path().join("objects"), 4)
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
