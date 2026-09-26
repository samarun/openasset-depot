use std::{pin::Pin, time::Duration};

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::error::AppResult;

use super::BlobManifest;

/// A readable chunk body. Boxed so backends can stream from disk or from a
/// network response without buffering the whole chunk in memory.
pub type ChunkReader = Pin<Box<dyn AsyncRead + Send + Unpin>>;

/// Raw byte storage for content-addressed chunks and blob manifests.
///
/// All chunking, hashing, and integrity logic lives in [`ObjectStore`], which
/// is backend-agnostic. A backend only has to store and retrieve immutable,
/// hash-named objects, which is exactly what both a POSIX filesystem and an
/// S3-compatible bucket provide.
///
/// Objects are immutable and named by their BLAKE3 hash, so `put_*` is safely
/// idempotent and backends may skip a write when the object already exists.
///
/// [`ObjectStore`]: super::ObjectStore
#[async_trait]
pub trait ChunkBackend: Send + Sync + std::fmt::Debug {
    async fn chunk_exists(&self, hash: &str) -> AppResult<bool>;

    async fn put_chunk(&self, hash: &str, bytes: &[u8]) -> AppResult<()>;

    async fn open_chunk(&self, hash: &str) -> AppResult<ChunkReader>;

    async fn manifest_exists(&self, hash: &str) -> AppResult<bool>;

    async fn put_manifest(&self, manifest: &BlobManifest) -> AppResult<()>;

    async fn read_manifest(&self, hash: &str) -> AppResult<BlobManifest>;

    /// Removes abandoned partial uploads older than `max_age`.
    ///
    /// Returns the number of objects reclaimed. Backends with no temporary
    /// staging area return zero.
    async fn cleanup_stale_uploads(&self, max_age: Duration) -> AppResult<usize>;

    /// Short human description used in admin output and startup logs.
    fn describe(&self) -> String;
}
