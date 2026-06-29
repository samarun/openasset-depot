use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeWorkspaceRequest {
    pub workspace_id: String,
    pub depot: String,
    pub stream: String,
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeWorkspace {
    pub workspace_id: String,
    pub depot: String,
    pub stream: String,
    pub root: PathBuf,
    pub active_changelist_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveWorkspaceRequest {
    pub workspace_id: String,
    pub root: String,
    pub delete_local_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveWorkspaceResult {
    pub removed_metadata: bool,
    pub removed_local_files: bool,
    pub root: PathBuf,
}

#[derive(Serialize)]
struct EmptyFileMap {
    files: std::collections::BTreeMap<String, serde_json::Value>,
}

#[tauri::command]
pub async fn initialize_workspace(
    request: InitializeWorkspaceRequest,
) -> Result<NativeWorkspace, String> {
    initialize_workspace_inner(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn remove_workspace_metadata(
    request: RemoveWorkspaceRequest,
) -> Result<RemoveWorkspaceResult, String> {
    remove_workspace_inner(request)
        .await
        .map_err(|error| error.to_string())
}

async fn initialize_workspace_inner(
    request: InitializeWorkspaceRequest,
) -> anyhow::Result<NativeWorkspace> {
    validate_identifier(&request.workspace_id, "workspace_id")?;
    let requested_root = PathBuf::from(&request.root);
    if requested_root.as_os_str().is_empty() {
        anyhow::bail!("workspace root is required");
    }
    fs::create_dir_all(&requested_root).await?;
    let root = fs::canonicalize(&requested_root).await?;
    if !root.is_dir() {
        anyhow::bail!("workspace root is not a directory");
    }

    let oad_dir = root.join(".oad");
    let config_path = oad_dir.join("workspace.json");
    if config_path.exists() {
        let existing: NativeWorkspace = read_json(&config_path).await?;
        if existing.workspace_id != request.workspace_id {
            anyhow::bail!(
                "directory already belongs to workspace {}",
                existing.workspace_id
            );
        }
        return Ok(existing);
    }

    fs::create_dir_all(&oad_dir).await?;
    let workspace = NativeWorkspace {
        workspace_id: request.workspace_id,
        depot: request.depot,
        stream: request.stream,
        root,
        active_changelist_id: None,
    };
    write_json_atomic(&config_path, &workspace).await?;
    let empty = EmptyFileMap {
        files: std::collections::BTreeMap::new(),
    };
    write_json_atomic(&oad_dir.join("state.json"), &empty).await?;
    write_json_atomic(&oad_dir.join("pending.json"), &empty).await?;
    Ok(workspace)
}

async fn remove_workspace_inner(
    request: RemoveWorkspaceRequest,
) -> anyhow::Result<RemoveWorkspaceResult> {
    validate_identifier(&request.workspace_id, "workspace_id")?;
    let root = fs::canonicalize(&request.root).await?;
    let metadata_path = root.join(".oad").join("workspace.json");
    let workspace: NativeWorkspace = read_json(&metadata_path).await?;
    if workspace.workspace_id != request.workspace_id {
        anyhow::bail!("workspace metadata does not match the requested workspace");
    }

    if request.delete_local_files {
        validate_destructive_root(&root)?;
        fs::remove_dir_all(&root).await?;
        return Ok(RemoveWorkspaceResult {
            removed_metadata: true,
            removed_local_files: true,
            root,
        });
    }

    fs::remove_dir_all(root.join(".oad")).await?;
    Ok(RemoveWorkspaceResult {
        removed_metadata: true,
        removed_local_files: false,
        root,
    })
}

fn validate_identifier(value: &str, label: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() || value.len() > 128 {
        anyhow::bail!("{label} is invalid");
    }
    Ok(())
}

fn validate_destructive_root(root: &Path) -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home directory is unavailable"))?;
    let home = home.canonicalize().unwrap_or(home);
    if root == Path::new("/") || root == home || root.parent().is_none() {
        anyhow::bail!("refusing to delete unsafe workspace root {}", root.display());
    }
    Ok(())
}

async fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> anyhow::Result<T> {
    let bytes = fs::read(path).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, bytes).await?;
    fs::rename(&temp_path, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn initializes_workspace_atomically_and_reopens_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        let request = InitializeWorkspaceRequest {
            workspace_id: "workspace-123".to_string(),
            depot: "depot-123".to_string(),
            stream: "stream-123".to_string(),
            root: root.to_string_lossy().to_string(),
        };

        let first = initialize_workspace_inner(request.clone()).await.unwrap();
        let second = initialize_workspace_inner(request).await.unwrap();
        assert_eq!(first.workspace_id, second.workspace_id);
        assert!(root.join(".oad/workspace.json").is_file());
        assert!(root.join(".oad/state.json").is_file());
        assert!(root.join(".oad/pending.json").is_file());
    }

    #[tokio::test]
    async fn refuses_to_reuse_directory_for_another_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        let base = InitializeWorkspaceRequest {
            workspace_id: "workspace-123".to_string(),
            depot: "depot-123".to_string(),
            stream: "stream-123".to_string(),
            root: root.to_string_lossy().to_string(),
        };
        initialize_workspace_inner(base.clone()).await.unwrap();
        let conflicting = InitializeWorkspaceRequest {
            workspace_id: "workspace-456".to_string(),
            ..base
        };
        let error = initialize_workspace_inner(conflicting).await.unwrap_err();
        assert!(error.to_string().contains("already belongs"));
    }

    #[tokio::test]
    async fn metadata_only_removal_preserves_project_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        let request = InitializeWorkspaceRequest {
            workspace_id: "workspace-123".to_string(),
            depot: "depot-123".to_string(),
            stream: "stream-123".to_string(),
            root: root.to_string_lossy().to_string(),
        };
        initialize_workspace_inner(request).await.unwrap();
        fs::write(root.join("scene.blend"), b"scene").await.unwrap();

        let result = remove_workspace_inner(RemoveWorkspaceRequest {
            workspace_id: "workspace-123".to_string(),
            root: root.to_string_lossy().to_string(),
            delete_local_files: false,
        })
        .await
        .unwrap();
        assert!(result.removed_metadata);
        assert!(!result.removed_local_files);
        assert!(root.join("scene.blend").is_file());
        assert!(!root.join(".oad").exists());
    }
}
