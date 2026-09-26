use std::time::Duration;

use async_trait::async_trait;
use aws_sdk_s3::{error::SdkError, operation::head_object::HeadObjectError, Client};

use crate::error::{AppError, AppResult};

use super::{
    backend::{ChunkBackend, ChunkReader},
    BlobManifest,
};

/// S3-compatible chunk storage (AWS S3, MinIO, Ceph RGW, Cloudflare R2).
///
/// Keys mirror the local layout — `chunks/ab/cd/<hash>` and
/// `blobs/ab/cd/<hash>.json` — so an operator can migrate between backends with
/// a plain bucket sync and no key rewriting.
///
/// Objects are immutable and content-addressed, so writes are unconditional
/// PUTs guarded by a cheap HEAD; S3 already gives us atomic object replacement,
/// which removes the need for the temporary-file dance the local backend uses.
#[derive(Debug, Clone)]
pub struct S3ChunkBackend {
    client: Client,
    bucket: String,
    prefix: String,
}

impl S3ChunkBackend {
    /// Builds a backend from the ambient AWS configuration chain.
    ///
    /// `endpoint_url` should be set for MinIO and other non-AWS
    /// implementations; when it is present path-style addressing is forced,
    /// because such deployments rarely have wildcard DNS for virtual-host
    /// style bucket names.
    pub async fn new(
        bucket: String,
        prefix: String,
        endpoint_url: Option<String>,
    ) -> AppResult<Self> {
        let base = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let mut builder = aws_sdk_s3::config::Builder::from(&base);
        if let Some(endpoint) = endpoint_url {
            builder = builder.endpoint_url(endpoint).force_path_style(true);
        }
        let client = Client::from_conf(builder.build());

        let backend = Self {
            client,
            bucket,
            prefix: prefix.trim_matches('/').to_string(),
        };
        backend.ensure_bucket_reachable().await?;
        Ok(backend)
    }

    /// Fails fast at startup rather than on the first artist upload.
    async fn ensure_bucket_reachable(&self) -> AppResult<()> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|error| {
                AppError::configuration(format!(
                    "object storage bucket {} is not reachable: {error}",
                    self.bucket
                ))
            })?;
        Ok(())
    }

    fn key(&self, kind: &str, name: &str) -> String {
        let hash = name.trim_end_matches(".json");
        let shard = format!("{}/{}", &hash[0..2], &hash[2..4]);
        if self.prefix.is_empty() {
            format!("{kind}/{shard}/{name}")
        } else {
            format!("{}/{kind}/{shard}/{name}", self.prefix)
        }
    }

    async fn object_exists(&self, key: &str) -> AppResult<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(SdkError::ServiceError(service))
                if matches!(service.err(), HeadObjectError::NotFound(_)) =>
            {
                Ok(false)
            }
            Err(error) => Err(AppError::Internal(format!(
                "object storage head failed for {key}: {error}"
            ))),
        }
    }

    async fn put_object(&self, key: &str, bytes: Vec<u8>) -> AppResult<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(bytes.into())
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("object storage write failed for {key}: {error}"))
            })?;
        Ok(())
    }
}

#[async_trait]
impl ChunkBackend for S3ChunkBackend {
    async fn chunk_exists(&self, hash: &str) -> AppResult<bool> {
        self.object_exists(&self.key("chunks", hash)).await
    }

    async fn put_chunk(&self, hash: &str, bytes: &[u8]) -> AppResult<()> {
        let key = self.key("chunks", hash);
        if self.object_exists(&key).await? {
            return Ok(());
        }
        self.put_object(&key, bytes.to_vec()).await
    }

    async fn open_chunk(&self, hash: &str) -> AppResult<ChunkReader> {
        let key = self.key("chunks", hash);
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("object storage read failed for {key}: {error}"))
            })?;
        Ok(Box::pin(output.body.into_async_read()))
    }

    async fn manifest_exists(&self, hash: &str) -> AppResult<bool> {
        self.object_exists(&self.key("blobs", &format!("{hash}.json")))
            .await
    }

    async fn put_manifest(&self, manifest: &BlobManifest) -> AppResult<()> {
        let key = self.key("blobs", &format!("{}.json", manifest.hash));
        if self.object_exists(&key).await? {
            return Ok(());
        }
        self.put_object(&key, serde_json::to_vec_pretty(manifest)?)
            .await
    }

    async fn read_manifest(&self, hash: &str) -> AppResult<BlobManifest> {
        let key = self.key("blobs", &format!("{hash}.json"));
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("object storage read failed for {key}: {error}"))
            })?;
        let data = output.body.collect().await.map_err(|error| {
            AppError::Internal(format!("object storage read failed for {key}: {error}"))
        })?;
        Ok(serde_json::from_slice(&data.into_bytes())?)
    }

    async fn cleanup_stale_uploads(&self, _max_age: Duration) -> AppResult<usize> {
        // Writes are single atomic PUTs, so there is no temporary staging area
        // to reclaim. Abandoned S3 multipart uploads are best handled by a
        // bucket lifecycle rule, which operators configure outside the server.
        Ok(0)
    }

    fn describe(&self) -> String {
        if self.prefix.is_empty() {
            format!("s3 bucket {}", self.bucket)
        } else {
            format!("s3 bucket {}/{}", self.bucket, self.prefix)
        }
    }
}
