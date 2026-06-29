use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader},
    process::Command,
    time::timeout,
};

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const LONG_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationRequest {
    operation_id: String,
    root: PathBuf,
    server_url: String,
    token: String,
    username: String,
    command: String,
    #[serde(default)]
    paths: Vec<String>,
    reason: Option<String>,
    adapter: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IntegrationEnvelope {
    protocol_version: u32,
    ok: bool,
    data: Value,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationProgressEvent {
    operation_id: String,
    command: String,
    phase: String,
    message: String,
    completed: Option<u64>,
    total: Option<u64>,
}

#[tauri::command]
pub async fn run_oad_integration(app: AppHandle, request: IntegrationRequest) -> Result<Value, String> {
    validate_request(&request)?;
    let executable = find_cli();
    let mut command = Command::new(&executable);
    command
        .arg("--server")
        .arg(&request.server_url)
        .arg("--cwd")
        .arg(&request.root)
        .arg("integration")
        .arg("--protocol-version")
        .arg("2")
        .arg(&request.command)
        .args(&request.paths)
        .current_dir(&request.root)
        .env("OAD_TOKEN", &request.token)
        .env("OAD_USERNAME", &request.username)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(reason) = &request.reason {
        command.arg("--reason").arg(reason);
    }
    if let Some(adapter) = &request.adapter {
        command.arg("--adapter").arg(adapter);
    }
    if let Some(description) = &request.description {
        command.arg("--description").arg(description);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", executable.display()))?;
    let stdout = child.stdout.take().ok_or("Could not capture oad output")?;
    let stderr = child.stderr.take().ok_or("Could not capture oad errors")?;
    let stdout_task = tokio::spawn(read_protocol_stream(
        stdout,
        app,
        request.operation_id.clone(),
        request.command.clone(),
    ));
    let stderr_task = tokio::spawn(read_bounded(stderr));
    let operation_timeout = if matches!(request.command.as_str(), "sync" | "submit") {
        LONG_TIMEOUT
    } else {
        DEFAULT_TIMEOUT
    };

    let status = match timeout(operation_timeout, child.wait()).await {
        Ok(result) => result.map_err(|error| format!("oad process failed: {error}"))?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(format!(
                "OpenAsset operation timed out after {} seconds",
                operation_timeout.as_secs()
            ));
        }
    };
    let (envelope, stdout_overflow) = stdout_task
        .await
        .map_err(|error| format!("oad output reader failed: {error}"))?
        .map_err(|error| format!("Could not read oad output: {error}"))?;
    let (stderr, stderr_overflow) = stderr_task
        .await
        .map_err(|error| format!("oad error reader failed: {error}"))?
        .map_err(|error| format!("Could not read oad errors: {error}"))?;
    if stdout_overflow || stderr_overflow {
        return Err("OpenAsset CLI output exceeded the 4 MiB safety limit".to_string());
    }

    let envelope = envelope.ok_or_else(|| {
        let detail = String::from_utf8_lossy(&stderr).trim().to_string();
        if detail.is_empty() {
            "OpenAsset CLI returned no result".to_string()
        } else {
            detail
        }
    })?;
    if envelope.protocol_version != 2 {
        return Err("OpenAsset CLI protocol version is not supported".to_string());
    }
    if !status.success() || !envelope.ok {
        return Err(envelope
            .error
            .unwrap_or_else(|| String::from_utf8_lossy(&stderr).trim().to_string()));
    }
    Ok(envelope.data)
}

fn validate_request(request: &IntegrationRequest) -> Result<(), String> {
    if request.operation_id.is_empty()
        || request.operation_id.len() > 128
        || !request
            .operation_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("Operation id is invalid".to_string());
    }
    if !request.root.is_absolute() || !request.root.is_dir() {
        return Err("Workspace root must be an existing absolute directory".to_string());
    }
    if request.token.trim().is_empty() || request.token.contains('\0') {
        return Err("An authenticated desktop session is required".to_string());
    }
    if !request.server_url.starts_with("http://") && !request.server_url.starts_with("https://") {
        return Err("Server URL must use HTTP or HTTPS".to_string());
    }
    let single_path = matches!(
        request.command.as_str(),
        "checkout" | "add" | "delete" | "lock" | "unlock" | "revert" | "history"
    );
    let multi_path = matches!(request.command.as_str(), "status" | "validate");
    let no_path = matches!(request.command.as_str(), "context" | "pending" | "sync" | "submit");
    if !single_path && !multi_path && !no_path {
        return Err(format!("Unsupported integration command: {}", request.command));
    }
    if single_path && request.paths.len() != 1 {
        return Err(format!("{} requires exactly one file", request.command));
    }
    if no_path && !request.paths.is_empty() {
        return Err(format!("{} does not accept file paths", request.command));
    }
    for path in &request.paths {
        if path.is_empty() || path.len() > 4096 || path.contains('\0') {
            return Err("File path is invalid".to_string());
        }
    }
    if request.reason.is_some()
        && !matches!(request.command.as_str(), "checkout" | "delete" | "lock")
    {
        return Err(format!("{} does not accept a reason", request.command));
    }
    if request.adapter.is_some() && request.command != "validate" {
        return Err(format!("{} does not accept an adapter", request.command));
    }
    if request.description.is_some() && request.command != "submit" {
        return Err(format!("{} does not accept a description", request.command));
    }
    Ok(())
}

fn find_cli() -> PathBuf {
    if let Some(configured) = env::var_os("OAD_CLI") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return path;
        }
    }
    if let Ok(current) = env::current_exe() {
        if let Some(parent) = current.parent() {
            let sibling = parent.join(if cfg!(windows) { "oad.exe" } else { "oad" });
            if sibling.is_file() {
                return sibling;
            }
            #[cfg(debug_assertions)]
            for ancestor in parent.ancestors().take(6) {
                let development = ancestor
                    .join("target")
                    .join("debug")
                    .join(if cfg!(windows) { "oad.exe" } else { "oad" });
                if development.is_file() {
                    return development;
                }
            }
        }
    }
    Path::new(if cfg!(windows) { "oad.exe" } else { "oad" }).to_path_buf()
}

async fn read_bounded<R>(mut reader: R) -> std::io::Result<(Vec<u8>, bool)>
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut overflow = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = MAX_OUTPUT_BYTES.saturating_sub(retained.len());
        if read <= remaining {
            retained.extend_from_slice(&buffer[..read]);
        } else {
            retained.extend_from_slice(&buffer[..remaining]);
            overflow = true;
        }
    }
    Ok((retained, overflow))
}

async fn read_protocol_stream<R>(
    reader: R,
    app: AppHandle,
    operation_id: String,
    command: String,
) -> std::io::Result<(Option<IntegrationEnvelope>, bool)>
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut retained_bytes = 0usize;
    let mut overflow = false;
    let mut result = None;

    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            break;
        }
        retained_bytes = retained_bytes.saturating_add(read);
        if retained_bytes > MAX_OUTPUT_BYTES {
            overflow = true;
            continue;
        }
        let value: Value = serde_json::from_str(line.trim()).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid protocol message: {error}"),
            )
        })?;
        match value.get("type").and_then(Value::as_str) {
            Some("progress") => {
                let event = OperationProgressEvent {
                    operation_id: operation_id.clone(),
                    command: command.clone(),
                    phase: value
                        .get("phase")
                        .and_then(Value::as_str)
                        .unwrap_or("working")
                        .to_string(),
                    message: value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Working")
                        .to_string(),
                    completed: value.get("completed").and_then(Value::as_u64),
                    total: value.get("total").and_then(Value::as_u64),
                };
                let _ = app.emit("oad://operation-progress", event);
            }
            Some("result") => {
                result = Some(serde_json::from_value(value).map_err(|error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("invalid protocol result: {error}"),
                    )
                })?);
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "unsupported protocol message",
                ));
            }
        }
    }
    Ok((result, overflow))
}

#[cfg(test)]
mod tests {
    use super::{read_bounded, validate_request, IntegrationRequest, MAX_OUTPUT_BYTES};
    use std::path::PathBuf;

    fn request(root: PathBuf, command: &str, paths: Vec<String>) -> IntegrationRequest {
        IntegrationRequest {
            operation_id: "test-operation".to_string(),
            root,
            server_url: "https://depot.example.test".to_string(),
            token: "test-token".to_string(),
            username: "artist".to_string(),
            command: command.to_string(),
            paths,
            reason: None,
            adapter: None,
            description: None,
        }
    }

    #[test]
    fn rejects_multiple_checkout_paths() {
        let directory = tempfile::tempdir().unwrap();
        let input = request(
            directory.path().to_path_buf(),
            "checkout",
            vec!["A.blend".to_string(), "B.blend".to_string()],
        );
        assert!(validate_request(&input).is_err());
    }

    #[tokio::test]
    async fn bounded_reader_drains_and_reports_overflow() {
        let payload = vec![b'x'; MAX_OUTPUT_BYTES + 1024];
        let (retained, overflow) = read_bounded(payload.as_slice()).await.unwrap();
        assert_eq!(retained.len(), MAX_OUTPUT_BYTES);
        assert!(overflow);
    }
}
