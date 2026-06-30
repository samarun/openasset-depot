use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use fs2::FileExt;
use reqwest::multipart::{Form, Part};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "oad", version, about = "OpenAsset Depot CLI")]
struct Cli {
    #[arg(long, global = true, env = "OAD_SERVER_URL")]
    server: Option<String>,
    #[arg(long, global = true, value_name = "DIRECTORY")]
    cwd: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Login(LoginArgs),
    Depot {
        #[command(subcommand)]
        command: DepotCommand,
    },
    Stream {
        #[command(subcommand)]
        command: StreamCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Sync,
    Status,
    Add(PathArg),
    Edit(PathArg),
    Delete(PathArg),
    Lock(LockArgs),
    Unlock(PathArg),
    Change {
        #[command(subcommand)]
        command: ChangeCommand,
    },
    Submit,
    Revert(PathArg),
    History(PathArg),
    Locks,
    Filetypes {
        #[command(subcommand)]
        command: FiletypeCommand,
    },
    Adapters {
        #[command(subcommand)]
        command: AdapterCommand,
    },
    Integration {
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=2))]
        protocol_version: u8,
        #[command(subcommand)]
        command: IntegrationCommand,
    },
    Validate(ValidateArgs),
}

#[derive(Args)]
struct LoginArgs {
    username: String,
    #[arg(long)]
    password: String,
}

#[derive(Subcommand)]
enum DepotCommand {
    Create { name: String },
    List,
}

#[derive(Subcommand)]
enum StreamCommand {
    Create {
        name: String,
        #[arg(long)]
        depot: String,
    },
    List,
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    Create {
        name: String,
        #[arg(long)]
        depot: String,
        #[arg(long)]
        stream: String,
        #[arg(long)]
        path: PathBuf,
    },
    List,
    Remove {
        #[arg(long)]
        delete_local_files: bool,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum ChangeCommand {
    Create { description: String },
}

#[derive(Subcommand)]
enum FiletypeCommand {
    List,
}

#[derive(Subcommand)]
enum AdapterCommand {
    List,
    Detect(AdapterDetectArgs),
    Scan(AdapterScanArgs),
    Validate(AdapterValidateArgs),
}

#[derive(Subcommand)]
enum IntegrationCommand {
    Context,
    Pending,
    Status {
        paths: Vec<PathBuf>,
    },
    Checkout {
        path: PathBuf,
        #[arg(long)]
        reason: Option<String>,
    },
    Add {
        path: PathBuf,
    },
    Delete {
        path: PathBuf,
        #[arg(long)]
        reason: Option<String>,
    },
    Lock {
        path: PathBuf,
        #[arg(long)]
        reason: Option<String>,
    },
    Unlock {
        path: PathBuf,
    },
    Revert {
        path: PathBuf,
    },
    Sync,
    Submit {
        #[arg(long, default_value = "Submitted from a DCC integration")]
        description: String,
    },
    Preview {
        path: PathBuf,
        #[arg(long)]
        image: PathBuf,
    },
    ReviewProxy {
        path: PathBuf,
        #[arg(long)]
        media: PathBuf,
    },
    History {
        path: PathBuf,
    },
    Validate {
        paths: Vec<PathBuf>,
        #[arg(long)]
        adapter: Option<String>,
    },
}

#[derive(Args)]
struct AdapterDetectArgs {
    #[arg(long)]
    root: Option<PathBuf>,
}

#[derive(Args)]
struct AdapterScanArgs {
    path: PathBuf,
    #[arg(long)]
    adapter: Option<String>,
}

#[derive(Args)]
struct AdapterValidateArgs {
    paths: Vec<PathBuf>,
    #[arg(long)]
    adapter: Option<String>,
}

#[derive(Args)]
struct PathArg {
    path: PathBuf,
}

#[derive(Args)]
struct LockArgs {
    path: PathBuf,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args)]
struct ValidateArgs {
    paths: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct GlobalConfig {
    server_url: String,
    token: Option<String>,
    username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceConfig {
    workspace_id: Uuid,
    depot: String,
    stream: String,
    root: PathBuf,
    active_changelist_id: Option<Uuid>,
}

impl WorkspaceConfig {
    fn embedded_stream_id(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.stream).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WorkspaceState {
    files: HashMap<String, FileState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileState {
    revision_number: i32,
    blob_hash: String,
    local_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PendingState {
    files: HashMap<String, PendingFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingFile {
    action: String,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    token: String,
    user: UserResponse,
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    username: String,
}

#[derive(Debug, Deserialize)]
struct DepotResponse {
    id: Uuid,
    name: String,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    id: Uuid,
    name: String,
    depot_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct WorkspaceResponse {
    id: Uuid,
    name: String,
    depot_id: Uuid,
    stream_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct DeleteWorkspaceResponse {
    id: Uuid,
    deleted: bool,
    released_locks: u64,
    abandoned_changelists: u64,
}

#[derive(Debug, Deserialize)]
struct ChangelistResponse {
    id: Uuid,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubmitResponse {
    revisions: Vec<SubmittedRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubmittedRevision {
    path: String,
    revision_number: i32,
    blob_hash: String,
    size_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncPlanEntry {
    path: String,
    revision_number: i32,
    blob_hash: String,
    size_bytes: i64,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    preview_available: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SyncOutcome {
    synced_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockResponse {
    id: Uuid,
    stream_id: Uuid,
    workspace_id: Uuid,
    depot_path: String,
    user_id: Uuid,
    reason: Option<String>,
    state: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct LockPageResponse {
    items: Vec<LockResponse>,
    next_before_created_at: Option<String>,
    next_before_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct FileTypeRule {
    name: String,
    rule_kind: String,
    extension: Option<String>,
    directory_prefix: Option<String>,
    asset_class: String,
    lock_required: bool,
    generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    revision_number: i32,
    blob_hash: String,
    size_bytes: i64,
    action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationResponse {
    warnings: Vec<ValidationMessage>,
    errors: Vec<ValidationMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationMessage {
    path: String,
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct AdapterDefinition {
    app_name: String,
    supported_file_types: Vec<String>,
    dependency_scan_strategy: String,
    preview_generation_strategy: String,
}

#[derive(Debug, Deserialize)]
struct ProjectDetection {
    app_name: String,
    matched_rule: String,
    confidence: f32,
}

#[derive(Debug, Serialize)]
struct AdapterFileInput {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DependencyScanResult {
    source_file: String,
    adapter_name: String,
    dependencies: Vec<AssetDependency>,
}

#[derive(Debug, Deserialize)]
struct AssetDependency {
    target_file: String,
    dependency_type: String,
    dependency_status: String,
    confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdapterValidationResponse {
    warnings: Vec<AdapterValidationMessage>,
    errors: Vec<AdapterValidationMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdapterValidationMessage {
    path: String,
    code: String,
    message: String,
    adapter_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(cwd) = &cli.cwd {
        let cwd = cwd
            .canonicalize()
            .with_context(|| format!("working directory does not exist: {}", cwd.display()))?;
        if !cwd.is_dir() {
            bail!("working directory is not a directory: {}", cwd.display());
        }
        std::env::set_current_dir(&cwd)?;
    }
    let mut config = load_global_config().await?;
    if let Some(server) = cli.server.as_deref() {
        config.server_url = server.trim_end_matches('/').to_string();
    }
    if let Ok(token) = std::env::var("OAD_TOKEN") {
        if !token.trim().is_empty() {
            config.token = Some(token);
        }
    }
    if let Ok(username) = std::env::var("OAD_USERNAME") {
        if !username.trim().is_empty() {
            config.username = Some(username);
        }
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(30))
        .user_agent(concat!("openasset-depot-cli/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to initialize the HTTP client")?;

    match cli.command {
        Command::Login(args) => login(&client, &mut config, args).await?,
        Command::Depot { command } => depot(&client, &config, command).await?,
        Command::Stream { command } => stream(&client, &config, command).await?,
        Command::Workspace { command } => workspace(&client, &config, command).await?,
        Command::Sync => sync(&client, &config).await?,
        Command::Status => status().await?,
        Command::Add(arg) => file_op(&client, &config, arg.path, "add").await?,
        Command::Edit(arg) => file_op(&client, &config, arg.path, "edit").await?,
        Command::Delete(arg) => file_op(&client, &config, arg.path, "delete").await?,
        Command::Lock(args) => lock(&client, &config, args).await?,
        Command::Unlock(arg) => unlock(&client, &config, arg.path).await?,
        Command::Change { command } => change(&client, &config, command).await?,
        Command::Submit => submit(&client, &config).await?,
        Command::Revert(arg) => revert(&client, &config, arg.path).await?,
        Command::History(arg) => history(&client, &config, arg.path).await?,
        Command::Locks => locks(&client, &config).await?,
        Command::Filetypes { command } => match command {
            FiletypeCommand::List => filetypes(&client, &config).await?,
        },
        Command::Adapters { command } => adapters(&client, &config, command).await?,
        Command::Integration {
            protocol_version,
            command,
        } => {
            let operation = integration_command_name(&command);
            let progress = IntegrationProgress::new(protocol_version, operation);
            progress.emit("starting", format!("Starting {operation}"), 5);
            match integration(&client, &config, command, Some(&progress)).await {
                Ok(data) => {
                    progress.emit("complete", format!("{operation} complete"), 100);
                    print_integration_result(protocol_version, true, data, None)
                }
                Err(error) => {
                    print_integration_result(
                        protocol_version,
                        false,
                        serde_json::Value::Null,
                        Some(error.to_string()),
                    );
                    std::process::exit(1);
                }
            }
        }
        Command::Validate(args) => validate(&client, &config, args).await?,
    }

    Ok(())
}

async fn login(client: &reqwest::Client, config: &mut GlobalConfig, args: LoginArgs) -> Result<()> {
    let response: LoginResponse = post_json(
        client,
        config,
        "/api/auth/login",
        &serde_json::json!({ "username": args.username, "password": args.password }),
    )
    .await?;
    config.token = Some(response.token);
    config.username = Some(response.user.username.clone());
    save_global_config(config).await?;
    println!("logged in as {}", response.user.username);
    Ok(())
}

async fn depot(
    client: &reqwest::Client,
    config: &GlobalConfig,
    command: DepotCommand,
) -> Result<()> {
    match command {
        DepotCommand::Create { name } => {
            let depot: DepotResponse = authed_post_json(
                client,
                config,
                "/api/depots",
                &serde_json::json!({ "name": name }),
            )
            .await?;
            println!("created depot {} ({})", depot.name, depot.id);
        }
        DepotCommand::List => {
            let depots: Vec<DepotResponse> = authed_get_json(client, config, "/api/depots").await?;
            for depot in depots {
                println!("{}\t{}", depot.id, depot.name);
            }
        }
    }
    Ok(())
}

async fn stream(
    client: &reqwest::Client,
    config: &GlobalConfig,
    command: StreamCommand,
) -> Result<()> {
    match command {
        StreamCommand::Create { name, depot } => {
            let stream: StreamResponse = authed_post_json(
                client,
                config,
                "/api/streams",
                &serde_json::json!({ "name": name, "depot": depot }),
            )
            .await?;
            println!("created stream {} ({})", stream.name, stream.id);
        }
        StreamCommand::List => {
            let streams: Vec<StreamResponse> =
                authed_get_json(client, config, "/api/streams").await?;
            for stream in streams {
                println!("{}\t{}\t{}", stream.id, stream.depot_id, stream.name);
            }
        }
    }
    Ok(())
}

async fn workspace(
    client: &reqwest::Client,
    config: &GlobalConfig,
    command: WorkspaceCommand,
) -> Result<()> {
    match command {
        WorkspaceCommand::Create {
            name,
            depot,
            stream,
            path,
        } => {
            fs::create_dir_all(&path).await?;
            let canonical = path.canonicalize().unwrap_or(path);
            let workspace: WorkspaceResponse = authed_post_json(
                client,
                config,
                "/api/workspaces",
                &serde_json::json!({
                    "name": name,
                    "depot": depot,
                    "stream": stream,
                    "local_path": canonical.to_string_lossy(),
                }),
            )
            .await?;
            let oad_dir = canonical.join(".oad");
            fs::create_dir_all(&oad_dir).await?;
            write_json(
                &oad_dir.join("workspace.json"),
                &WorkspaceConfig {
                    workspace_id: workspace.id,
                    depot,
                    stream,
                    root: canonical.clone(),
                    active_changelist_id: None,
                },
            )
            .await?;
            write_json(&oad_dir.join("state.json"), &WorkspaceState::default()).await?;
            write_json(&oad_dir.join("pending.json"), &PendingState::default()).await?;
            println!(
                "created workspace {} ({}) at {}",
                workspace.name,
                workspace.id,
                canonical.display()
            );
        }
        WorkspaceCommand::List => {
            let workspaces: Vec<WorkspaceResponse> =
                authed_get_json(client, config, "/api/workspaces").await?;
            for workspace in workspaces {
                println!(
                    "{}\t{}\t{}\t{}",
                    workspace.id, workspace.depot_id, workspace.stream_id, workspace.name
                );
            }
        }
        WorkspaceCommand::Remove {
            delete_local_files,
            force,
        } => {
            remove_workspace(client, config, delete_local_files, force).await?;
        }
    }
    Ok(())
}

async fn remove_workspace(
    client: &reqwest::Client,
    config: &GlobalConfig,
    delete_local_files: bool,
    force: bool,
) -> Result<()> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let pending = load_pending(&workspace_dir).await?;
    if !pending.files.is_empty() && !force {
        bail!(
            "workspace has {} pending file operation(s); submit/revert them or pass --force",
            pending.files.len()
        );
    }

    let response: DeleteWorkspaceResponse = authed_delete_json(
        client,
        config,
        &format!("/api/workspaces/{}", workspace.workspace_id),
    )
    .await?;
    if !response.deleted || response.id != workspace.workspace_id {
        bail!("server did not confirm workspace removal");
    }

    drop(_operation_lock);
    if delete_local_files {
        validate_workspace_removal_target(&workspace.root, workspace.workspace_id).await?;
        fs::remove_dir_all(&workspace.root)
            .await
            .with_context(|| format!("failed to remove {}", workspace.root.display()))?;
        println!("removed workspace files at {}", workspace.root.display());
    } else {
        fs::remove_dir_all(&workspace_dir)
            .await
            .with_context(|| format!("failed to remove {}", workspace_dir.display()))?;
        println!(
            "removed workspace metadata; local project files remain at {}",
            workspace.root.display()
        );
    }
    println!(
        "released {} lock(s), abandoned {} pending changelist(s)",
        response.released_locks, response.abandoned_changelists
    );
    Ok(())
}

async fn validate_workspace_removal_target(root: &Path, expected_id: Uuid) -> Result<()> {
    let canonical = root
        .canonicalize()
        .with_context(|| format!("workspace root does not exist: {}", root.display()))?;
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not determine home directory"))?;
    let home = home.canonicalize().unwrap_or(home);
    if canonical == Path::new("/") || canonical == home || canonical.parent().is_none() {
        bail!(
            "refusing to delete unsafe workspace root {}",
            canonical.display()
        );
    }
    let metadata_path = canonical.join(".oad").join("workspace.json");
    let recorded: WorkspaceConfig = read_json(&metadata_path).await.with_context(|| {
        format!(
            "missing valid workspace metadata at {}",
            metadata_path.display()
        )
    })?;
    if recorded.workspace_id != expected_id {
        bail!("workspace metadata does not match the server workspace id");
    }
    Ok(())
}

async fn sync(client: &reqwest::Client, config: &GlobalConfig) -> Result<()> {
    let outcome = sync_workspace(client, config, None).await?;
    println!("synced {} file(s)", outcome.synced_count);
    Ok(())
}

async fn sync_workspace(
    client: &reqwest::Client,
    config: &GlobalConfig,
    progress: Option<&IntegrationProgress>,
) -> Result<SyncOutcome> {
    const PAGE_SIZE: usize = 1_000;
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let mut state = load_state(&workspace_dir).await?;
    let pending = load_pending(&workspace_dir).await?;
    let temp_dir = workspace_dir.join("tmp");
    fs::create_dir_all(&temp_dir).await?;
    let force_full = state.files.is_empty();
    let mut after_path: Option<String> = None;
    let mut synced_count = 0usize;
    if let Some(progress) = progress {
        progress.emit("planning", "Checking the depot for updates", 15);
    }
    let sync_result: Result<()> = async {
        loop {
            let plan: Vec<SyncPlanEntry> = authed_post_json(
                client,
                config,
                "/api/sync/plan",
                &serde_json::json!({
                    "workspace_id": workspace.workspace_id,
                    "after_path": after_path,
                    "limit": PAGE_SIZE,
                    "force_full": force_full,
                }),
            )
            .await?;
            if plan.is_empty() {
                break;
            }
            let page_len = plan.len();
            let last_path = plan.last().map(|entry| entry.path.clone());
            let mut acknowledgements = Vec::with_capacity(page_len);

            for entry in plan {
                if pending.files.contains_key(&entry.path) {
                    bail!(
                        "sync would overwrite pending work in {}; submit or revert it first",
                        entry.path
                    );
                }
                let current = state.files.get(&entry.path).cloned();
                let target = safe_workspace_target(&workspace.root, &entry.path)?;
                if entry.deleted {
                    if target.exists() {
                        match current.as_ref() {
                            Some(file) if hash_file(&target).await? == file.local_hash => {
                                fs::remove_file(&target).await.with_context(|| {
                                    format!("failed to remove deleted depot file {}", target.display())
                                })?;
                            }
                            Some(_) => bail!(
                                "sync would remove locally modified file {}; submit or revert it first",
                                entry.path
                            ),
                            None => bail!(
                                "sync would remove untracked file {}; move or add it first",
                                entry.path
                            ),
                        }
                    }
                    state.files.remove(&entry.path);
                } else if current.as_ref().map(|file| file.revision_number)
                    == Some(entry.revision_number)
                    && target.is_file()
                {
                    let expected = current.as_ref().map(|file| file.local_hash.as_str());
                    if hash_file(&target).await?.as_str() != expected.unwrap_or_default() {
                        bail!(
                            "sync found local modifications in {}; submit or revert them first",
                            entry.path
                        );
                    }
                } else {
                    let mut local_hash = None;
                    if target.exists() {
                        let hash = hash_file(&target).await?;
                        match current.as_ref() {
                            Some(file) if hash != file.local_hash => bail!(
                                "sync would overwrite local modifications in {}; submit or revert them first",
                                entry.path
                            ),
                            None if hash != entry.blob_hash => bail!(
                                "sync would overwrite untracked file {}; move or add it first",
                                entry.path
                            ),
                            _ => {}
                        }
                        if hash == entry.blob_hash {
                            local_hash = Some(hash);
                        }
                    }
                    let local_hash = match local_hash {
                        Some(hash) => hash,
                        None => download_sync_entry(
                            client,
                            config,
                            workspace.workspace_id,
                            &workspace.root,
                            &temp_dir,
                            &entry,
                        )
                        .await?,
                    };
                    state.files.insert(
                        entry.path.clone(),
                        FileState {
                            revision_number: entry.revision_number,
                            blob_hash: entry.blob_hash.clone(),
                            local_hash,
                        },
                    );
                }
                acknowledgements.push(serde_json::json!({
                    "path": entry.path,
                    "revision_number": entry.revision_number,
                }));
                synced_count += 1;
            }

            save_state(&workspace_dir, &state).await?;
            if let Some(progress) = progress {
                progress.emit(
                    "transferring",
                    format!("Applied {synced_count} file updates"),
                    70,
                );
            }
            let _: serde_json::Value = authed_post_json(
                client,
                config,
                "/api/sync/ack",
                &serde_json::json!({
                    "workspace_id": workspace.workspace_id,
                    "entries": acknowledgements,
                }),
            )
            .await?;
            after_path = last_path;
            if page_len < PAGE_SIZE {
                break;
            }
        }
        Ok(())
    }
    .await;
    let _ = fs::remove_dir(&temp_dir).await;
    sync_result?;
    if let Some(progress) = progress {
        progress.emit("finalizing", "Recording the synced workspace state", 92);
    }
    Ok(SyncOutcome { synced_count })
}

async fn download_sync_entry(
    client: &reqwest::Client,
    config: &GlobalConfig,
    workspace_id: Uuid,
    workspace_root: &Path,
    temp_dir: &Path,
    entry: &SyncPlanEntry,
) -> Result<String> {
    let response = authed_request(config, client.post(api_url(config, "/api/sync/download")))?
        .json(&serde_json::json!({ "workspace_id": workspace_id, "path": entry.path }))
        .send()
        .await?;
    let mut response = ensure_success(response).await?;
    let target = safe_workspace_target(workspace_root, &entry.path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp = temp_dir.join(Uuid::new_v4().to_string());
    let result: Result<String> = async {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .await?;
        let mut hasher = blake3::Hasher::new();
        while let Some(chunk) = response.chunk().await? {
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        let local_hash = hasher.finalize().to_hex().to_string();
        if local_hash != entry.blob_hash {
            bail!(
                "integrity check failed while syncing {}: expected {}, received {}",
                entry.path,
                entry.blob_hash,
                local_hash
            );
        }
        replace_workspace_file(&tmp, &target, temp_dir).await?;
        Ok(local_hash)
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&tmp).await;
    }
    result
}

async fn replace_workspace_file(source: &Path, target: &Path, _temp_dir: &Path) -> Result<()> {
    if !target.exists() {
        fs::rename(source, target).await?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, target).await?;
        Ok(())
    }
    #[cfg(windows)]
    {
        let backup = _temp_dir.join(format!("{}.backup", Uuid::new_v4()));
        fs::rename(target, &backup).await?;
        if let Err(error) = fs::rename(source, target).await {
            let _ = fs::rename(&backup, target).await;
            return Err(error.into());
        }
        let _ = fs::remove_file(backup).await;
        Ok(())
    }
}

async fn status() -> Result<()> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let state = load_state(&workspace_dir).await?;
    let pending = load_pending(&workspace_dir).await?;
    let mut seen = HashSet::new();
    for entry in WalkDir::new(&workspace.root)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_dir() {
            if entry.file_name() == ".oad" {
                continue;
            }
            continue;
        }
        let path = entry.path();
        if path
            .components()
            .any(|component| component.as_os_str() == ".oad")
        {
            continue;
        }
        let depot_path = depot_path_for(&workspace.root, path)?;
        seen.insert(depot_path.clone());
        if let Some(pending) = pending.files.get(&depot_path) {
            println!("{}\t{}", pending.action, depot_path);
            continue;
        }
        match state.files.get(&depot_path) {
            None => println!("untracked\t{}", depot_path),
            Some(file_state) => {
                let hash = hash_file(path).await?;
                if hash != file_state.local_hash {
                    println!("modified\t{}", depot_path);
                }
            }
        }
    }
    for path in state.files.keys() {
        if !seen.contains(path) {
            println!("missing\t{}", path);
        }
    }
    Ok(())
}

async fn file_op(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
    action: &str,
) -> Result<()> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let endpoint = match action {
        "add" => "/api/files/add",
        "delete" => "/api/files/delete",
        _ => "/api/files/edit",
    };
    let _value: serde_json::Value = authed_post_json(
        client,
        config,
        endpoint,
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "changelist_id": workspace.active_changelist_id,
            "path": depot_path,
        }),
    )
    .await?;
    let mut pending = load_pending(&workspace_dir).await?;
    pending.files.insert(
        depot_path.clone(),
        PendingFile {
            action: action.to_string(),
        },
    );
    save_pending(&workspace_dir, &pending).await?;
    println!("{} {}", action, depot_path);
    Ok(())
}

async fn lock(client: &reqwest::Client, config: &GlobalConfig, args: LockArgs) -> Result<()> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &args.path)?;
    let response: LockResponse = authed_post_json(
        client,
        config,
        "/api/files/lock",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "path": depot_path,
            "reason": args.reason,
        }),
    )
    .await?;
    println!("locked {}\t{}", response.depot_path, response.state);
    Ok(())
}

async fn unlock(client: &reqwest::Client, config: &GlobalConfig, path: PathBuf) -> Result<()> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let response: LockResponse = authed_post_json(
        client,
        config,
        "/api/files/unlock",
        &serde_json::json!({ "workspace_id": workspace.workspace_id, "path": depot_path }),
    )
    .await?;
    println!("unlocked {}\t{}", response.depot_path, response.state);
    Ok(())
}

async fn change(
    client: &reqwest::Client,
    config: &GlobalConfig,
    command: ChangeCommand,
) -> Result<()> {
    match command {
        ChangeCommand::Create { description } => {
            let (mut workspace, workspace_dir) = load_workspace().await?;
            let _operation_lock = lock_workspace_operation(&workspace_dir)?;
            let response: ChangelistResponse = authed_post_json(
                client,
                config,
                "/api/changelists",
                &serde_json::json!({ "workspace_id": workspace.workspace_id, "description": description }),
            )
            .await?;
            workspace.active_changelist_id = Some(response.id);
            save_workspace(&workspace_dir, &workspace).await?;
            println!("created changelist {} ({})", response.id, response.status);
        }
    }
    Ok(())
}

async fn submit(client: &reqwest::Client, config: &GlobalConfig) -> Result<()> {
    let response = submit_workspace(client, config, None, None).await?;
    for revision in response.revisions {
        println!("submitted #{}\t{}", revision.revision_number, revision.path);
    }
    Ok(())
}

async fn submit_workspace(
    client: &reqwest::Client,
    config: &GlobalConfig,
    create_description: Option<&str>,
    progress: Option<&IntegrationProgress>,
) -> Result<SubmitResponse> {
    let (mut workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let mut pending = load_pending(&workspace_dir).await?;
    if pending.files.is_empty() {
        bail!("nothing to submit");
    }

    let changelist_id = match workspace.active_changelist_id {
        Some(id) => id,
        None => {
            let description = create_description.ok_or_else(|| {
                anyhow!("no active changelist; run `oad change create \"description\"`")
            })?;
            let change: ChangelistResponse = authed_post_json(
                client,
                config,
                "/api/changelists",
                &serde_json::json!({
                    "workspace_id": workspace.workspace_id,
                    "description": description,
                }),
            )
            .await?;
            workspace.active_changelist_id = Some(change.id);
            save_workspace(&workspace_dir, &workspace).await?;
            change.id
        }
    };

    let mut paths = pending.files.keys().cloned().collect::<Vec<_>>();
    paths.sort();
    if let Some(progress) = progress {
        progress.emit("preparing", format!("Preparing {} files", paths.len()), 20);
    }
    let mut validation_entries = Vec::with_capacity(paths.len());
    let mut state = load_state(&workspace_dir).await?;
    for path in &paths {
        let action = &pending.files[path].action;
        let local_path = safe_workspace_target(&workspace.root, path)?;
        if action == "delete" {
            if let Some(tracked) = state.files.get(path) {
                if local_path.exists() && hash_file(&local_path).await? != tracked.local_hash {
                    bail!(
                        "refusing to delete locally modified file {}; revert or submit its edit first",
                        path
                    );
                }
            }
        } else {
            let metadata = fs::metadata(&local_path)
                .await
                .with_context(|| format!("pending file is missing: {}", local_path.display()))?;
            validation_entries.push(serde_json::json!({
                "path": path,
                "size_bytes": metadata.len(),
            }));
        }
        let endpoint = match action.as_str() {
            "add" => "/api/files/add",
            "delete" => "/api/files/delete",
            _ => "/api/files/edit",
        };
        let _: serde_json::Value = authed_post_json(
            client,
            config,
            endpoint,
            &serde_json::json!({
                "workspace_id": workspace.workspace_id,
                "changelist_id": changelist_id,
                "path": path,
            }),
        )
        .await?;
    }

    if let Some(progress) = progress {
        progress.emit("validating", "Checking locks, rules, and dependencies", 42);
    }
    let validation: ValidationResponse = authed_post_json(
        client,
        config,
        "/api/validate",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "paths": validation_entries,
        }),
    )
    .await?;
    if !validation.errors.is_empty() {
        let messages = validation
            .errors
            .iter()
            .map(|message| format!("{}: {}", message.path, message.message))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("submit validation failed: {messages}");
    }

    let mut form = Form::new().text("workspace_id", workspace.workspace_id.to_string());
    for path in &paths {
        if pending.files[path].action == "delete" {
            continue;
        }
        let local_path = safe_workspace_target(&workspace.root, path)?;
        let file = fs::File::open(&local_path)
            .await
            .with_context(|| format!("failed to open {}", local_path.display()))?;
        let stream = ReaderStream::new(file);
        let part = Part::stream(reqwest::Body::wrap_stream(stream)).file_name(path.clone());
        form = form.part("file", part);
    }

    if let Some(progress) = progress {
        progress.emit("uploading", "Uploading changed asset data", 62);
    }
    let response = authed_request(
        config,
        client
            .post(api_url(
                config,
                &format!("/api/changelists/{changelist_id}/submit"),
            ))
            .multipart(form),
    )?
    .send()
    .await?;
    let response: SubmitResponse = parse_response(response).await?;

    if let Some(progress) = progress {
        progress.emit("finalizing", "Updating local revision records", 88);
    }
    for revision in &response.revisions {
        let local_path = safe_workspace_target(&workspace.root, &revision.path)?;
        if pending
            .files
            .get(&revision.path)
            .map(|file| file.action.as_str())
            == Some("delete")
        {
            if local_path.exists() {
                fs::remove_file(&local_path).await.with_context(|| {
                    format!("failed to remove deleted file {}", local_path.display())
                })?;
            }
            state.files.remove(&revision.path);
            continue;
        }
        let local_hash = hash_file(&local_path).await?;
        state.files.insert(
            revision.path.clone(),
            FileState {
                revision_number: revision.revision_number,
                blob_hash: revision.blob_hash.clone(),
                local_hash,
            },
        );
    }
    save_state(&workspace_dir, &state).await?;
    pending.files.clear();
    save_pending(&workspace_dir, &pending).await?;
    workspace.active_changelist_id = None;
    save_workspace(&workspace_dir, &workspace).await?;
    Ok(response)
}

async fn revert(client: &reqwest::Client, config: &GlobalConfig, path: PathBuf) -> Result<()> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let mut pending = load_pending(&workspace_dir).await?;
    pending.files.remove(&depot_path);
    save_pending(&workspace_dir, &pending).await?;
    let _value: serde_json::Value = authed_post_json(
        client,
        config,
        "/api/files/revert",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "changelist_id": workspace.active_changelist_id,
            "path": depot_path,
        }),
    )
    .await?;
    println!("reverted {}", depot_path);
    Ok(())
}

async fn history(client: &reqwest::Client, config: &GlobalConfig, path: PathBuf) -> Result<()> {
    let (workspace, _) = load_workspace().await?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let url = format!(
        "/api/files/history?workspace_id={}&path={}",
        workspace.workspace_id,
        urlencoding::encode(&depot_path)
    );
    let entries: Vec<HistoryEntry> = authed_get_json(client, config, &url).await?;
    for entry in entries {
        println!(
            "#{}\t{}\t{} bytes\t{}",
            entry.revision_number, entry.action, entry.size_bytes, entry.blob_hash
        );
    }
    Ok(())
}

async fn locks(client: &reqwest::Client, config: &GlobalConfig) -> Result<()> {
    let locks = list_locks_paged(client, config, None, None).await?;
    for lock in locks {
        println!(
            "{}\t{}\t{}\t{}",
            lock.depot_path,
            lock.state,
            lock.user_id,
            lock.reason.unwrap_or_default()
        );
    }
    Ok(())
}

async fn filetypes(client: &reqwest::Client, config: &GlobalConfig) -> Result<()> {
    let rules: Vec<FileTypeRule> = authed_get_json(client, config, "/api/filetypes").await?;
    for rule in rules {
        let pattern = rule
            .extension
            .or(rule.directory_prefix)
            .unwrap_or_else(|| rule.rule_kind.clone());
        println!(
            "{}\t{}\t{}\tlock_required={}\tgenerated={}",
            pattern, rule.asset_class, rule.name, rule.lock_required, rule.generated
        );
    }
    Ok(())
}

async fn adapters(
    client: &reqwest::Client,
    config: &GlobalConfig,
    command: AdapterCommand,
) -> Result<()> {
    match command {
        AdapterCommand::List => adapter_list(client, config).await,
        AdapterCommand::Detect(args) => adapter_detect(client, config, args).await,
        AdapterCommand::Scan(args) => adapter_scan(client, config, args).await,
        AdapterCommand::Validate(args) => adapter_validate(client, config, args).await,
    }
}

async fn adapter_list(client: &reqwest::Client, config: &GlobalConfig) -> Result<()> {
    let adapters: Vec<AdapterDefinition> = authed_get_json(client, config, "/api/adapters").await?;
    for adapter in adapters {
        println!(
            "{}\t{}\t{}\t{}",
            adapter.app_name,
            adapter.supported_file_types.join(","),
            adapter.dependency_scan_strategy,
            adapter.preview_generation_strategy
        );
    }
    Ok(())
}

async fn adapter_detect(
    client: &reqwest::Client,
    config: &GlobalConfig,
    args: AdapterDetectArgs,
) -> Result<()> {
    let root = match args.root {
        Some(root) => root.canonicalize().unwrap_or(root),
        None => load_workspace().await?.0.root,
    };
    let relative_paths = workspace_files(&root)?;
    let detections: Vec<ProjectDetection> = authed_post_json(
        client,
        config,
        "/api/adapters/detect",
        &serde_json::json!({
            "root_path": root.to_string_lossy(),
            "relative_paths": relative_paths,
        }),
    )
    .await?;
    for detection in detections {
        println!(
            "{}\tconfidence={:.2}\t{}",
            detection.app_name, detection.confidence, detection.matched_rule
        );
    }
    Ok(())
}

async fn adapter_scan(
    client: &reqwest::Client,
    config: &GlobalConfig,
    args: AdapterScanArgs,
) -> Result<()> {
    let (workspace, _) = load_workspace().await?;
    let depot_path = depot_path_for_input(&workspace.root, &args.path)?;
    let file = adapter_file_input(&workspace.root, depot_path).await?;
    let scan: DependencyScanResult = authed_post_json(
        client,
        config,
        "/api/adapters/scan",
        &serde_json::json!({
            "adapter_name": args.adapter,
            "file": file,
        }),
    )
    .await?;
    println!("{}\t{}", scan.adapter_name, scan.source_file);
    if scan.dependencies.is_empty() {
        println!("no dependencies found");
    }
    for dependency in scan.dependencies {
        println!(
            "{}\t{}\t{}\tconfidence={:.2}",
            dependency.dependency_type,
            dependency.dependency_status,
            dependency.target_file,
            dependency.confidence
        );
    }
    Ok(())
}

async fn adapter_validate(
    client: &reqwest::Client,
    config: &GlobalConfig,
    args: AdapterValidateArgs,
) -> Result<()> {
    let (workspace, _) = load_workspace().await?;
    let paths = if args.paths.is_empty() {
        workspace_files(&workspace.root)?
    } else {
        args.paths
            .iter()
            .map(|path| depot_path_for_input(&workspace.root, path))
            .collect::<Result<Vec<_>>>()?
    };
    let mut files = Vec::new();
    for path in paths {
        files.push(adapter_file_input(&workspace.root, path).await?);
    }
    let response: AdapterValidationResponse = authed_post_json(
        client,
        config,
        "/api/adapters/validate",
        &serde_json::json!({
            "adapter_name": args.adapter,
            "files": files,
        }),
    )
    .await?;
    for warning in response.warnings {
        println!(
            "warning\t{}\t{}\t{}\t{}",
            warning.adapter_name, warning.code, warning.path, warning.message
        );
    }
    for error in response.errors {
        println!(
            "error\t{}\t{}\t{}\t{}",
            error.adapter_name, error.code, error.path, error.message
        );
    }
    Ok(())
}

async fn validate(
    client: &reqwest::Client,
    config: &GlobalConfig,
    args: ValidateArgs,
) -> Result<()> {
    let (workspace, _) = load_workspace().await?;
    let paths = if args.paths.is_empty() {
        workspace_files(&workspace.root)?
    } else {
        args.paths
            .iter()
            .map(|path| depot_path_for_input(&workspace.root, path))
            .collect::<Result<Vec<_>>>()?
    };
    let mut entries = Vec::new();
    let set: HashSet<_> = paths.iter().cloned().collect();
    for path in &paths {
        let local_path = workspace.root.join(path);
        let size_bytes = fs::metadata(&local_path).await.ok().map(|meta| meta.len());
        if is_unity_asset_needing_meta(path) && !set.contains(&format!("{path}.meta")) {
            println!("warning\tunity_meta_missing\t{} missing .meta", path);
        }
        entries.push(serde_json::json!({ "path": path, "size_bytes": size_bytes }));
    }
    let response: ValidationResponse = authed_post_json(
        client,
        config,
        "/api/validate",
        &serde_json::json!({ "workspace_id": workspace.workspace_id, "paths": entries }),
    )
    .await?;
    for warning in response.warnings {
        println!(
            "warning\t{}\t{}\t{}",
            warning.code, warning.path, warning.message
        );
    }
    for error in response.errors {
        println!("error\t{}\t{}\t{}", error.code, error.path, error.message);
    }
    Ok(())
}

struct IntegrationProgress {
    protocol_version: u8,
    operation: &'static str,
}

impl IntegrationProgress {
    fn new(protocol_version: u8, operation: &'static str) -> Self {
        Self {
            protocol_version,
            operation,
        }
    }

    fn emit(&self, phase: &str, message: impl Into<String>, completed: u8) {
        if self.protocol_version != 2 {
            return;
        }
        print_protocol_line(serde_json::json!({
            "protocol_version": 2,
            "type": "progress",
            "operation": self.operation,
            "phase": phase,
            "message": message.into(),
            "completed": completed,
            "total": 100,
        }));
    }
}

fn integration_command_name(command: &IntegrationCommand) -> &'static str {
    match command {
        IntegrationCommand::Sync => "sync",
        IntegrationCommand::Submit { .. } => "submit",
        IntegrationCommand::Preview { .. } => "preview",
        IntegrationCommand::ReviewProxy { .. } => "review_proxy",
        IntegrationCommand::Context => "context",
        IntegrationCommand::Pending => "pending",
        IntegrationCommand::Status { .. } => "status",
        IntegrationCommand::Checkout { .. } => "checkout",
        IntegrationCommand::Add { .. } => "add",
        IntegrationCommand::Delete { .. } => "delete",
        IntegrationCommand::Lock { .. } => "lock",
        IntegrationCommand::Unlock { .. } => "unlock",
        IntegrationCommand::Revert { .. } => "revert",
        IntegrationCommand::History { .. } => "history",
        IntegrationCommand::Validate { .. } => "validate",
    }
}

fn print_integration_result(
    protocol_version: u8,
    ok: bool,
    data: serde_json::Value,
    error: Option<String>,
) {
    let mut result = serde_json::json!({
        "protocol_version": protocol_version,
        "ok": ok,
        "data": data,
        "error": error,
    });
    if protocol_version == 2 {
        result["type"] = serde_json::Value::String("result".to_string());
    }
    print_protocol_line(result);
}

fn print_protocol_line(value: serde_json::Value) {
    use std::io::Write as _;
    println!("{value}");
    let _ = std::io::stdout().flush();
}

async fn integration(
    client: &reqwest::Client,
    config: &GlobalConfig,
    command: IntegrationCommand,
    progress: Option<&IntegrationProgress>,
) -> Result<serde_json::Value> {
    match command {
        IntegrationCommand::Context => integration_context(config).await,
        IntegrationCommand::Pending => integration_pending().await,
        IntegrationCommand::Status { paths } => integration_status(client, config, paths).await,
        IntegrationCommand::Checkout { path, reason } => {
            integration_checkout(client, config, path, reason).await
        }
        IntegrationCommand::Add { path } => integration_add(client, config, path).await,
        IntegrationCommand::Delete { path, reason } => {
            integration_delete(client, config, path, reason).await
        }
        IntegrationCommand::Lock { path, reason } => {
            integration_lock(client, config, path, reason).await
        }
        IntegrationCommand::Unlock { path } => integration_unlock(client, config, path).await,
        IntegrationCommand::Revert { path } => integration_revert(client, config, path).await,
        IntegrationCommand::Sync => Ok(serde_json::to_value(
            sync_workspace(client, config, progress).await?,
        )?),
        IntegrationCommand::Submit { description } => {
            let response = submit_workspace(client, config, Some(&description), progress).await?;
            Ok(serde_json::to_value(response)?)
        }
        IntegrationCommand::Preview { path, image } => {
            integration_preview(client, config, path, image).await
        }
        IntegrationCommand::ReviewProxy { path, media } => {
            integration_review_proxy(client, config, path, media).await
        }
        IntegrationCommand::History { path } => integration_history(client, config, path).await,
        IntegrationCommand::Validate { paths, adapter } => {
            integration_validate(client, config, paths, adapter).await
        }
    }
}

async fn integration_context(config: &GlobalConfig) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let state = load_state(&workspace_dir).await?;
    let pending = load_pending(&workspace_dir).await?;
    Ok(serde_json::json!({
        "workspace_id": workspace.workspace_id,
        "depot": workspace.depot,
        "stream": workspace.stream,
        "root": workspace.root,
        "active_changelist_id": workspace.active_changelist_id,
        "tracked_file_count": state.files.len(),
        "pending_file_count": pending.files.len(),
        "server_url": config.server_url,
        "username": config.username,
        "authenticated": config.token.is_some(),
    }))
}

async fn integration_pending() -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let pending = load_pending(&workspace_dir).await?;
    let mut files = pending
        .files
        .into_iter()
        .map(|(path, file)| {
            let local_path = safe_workspace_target(&workspace.root, &path)?;
            Ok(serde_json::json!({
                "path": path,
                "local_path": local_path,
                "action": file.action,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    files.sort_by(|left, right| {
        left["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["path"].as_str().unwrap_or_default())
    });
    Ok(serde_json::json!({
        "changelist_id": workspace.active_changelist_id,
        "files": files,
    }))
}

async fn integration_status(
    client: &reqwest::Client,
    config: &GlobalConfig,
    paths: Vec<PathBuf>,
) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let state = load_state(&workspace_dir).await?;
    let pending = load_pending(&workspace_dir).await?;
    let stream_id = resolve_workspace_stream_id(client, config, &workspace).await?;
    let locks = list_locks_paged(client, config, Some(stream_id), None).await?;

    let lock_by_path: HashMap<_, _> = locks
        .into_iter()
        .filter(|lock| lock.stream_id == stream_id)
        .map(|lock| (lock.depot_path.clone(), lock))
        .collect();
    let mut selected = if paths.is_empty() {
        let mut values: HashSet<String> = workspace_files(&workspace.root)?.into_iter().collect();
        values.extend(state.files.keys().cloned());
        values.extend(pending.files.keys().cloned());
        values.into_iter().collect::<Vec<_>>()
    } else {
        paths
            .iter()
            .map(|path| depot_path_for_input(&workspace.root, path))
            .collect::<Result<Vec<_>>>()?
    };
    selected.sort();
    selected.dedup();

    let mut remote_by_path = HashMap::with_capacity(selected.len());
    for chunk in selected.chunks(5_000) {
        let plan: Vec<SyncPlanEntry> = authed_post_json(
            client,
            config,
            "/api/sync/plan",
            &serde_json::json!({
                "workspace_id": workspace.workspace_id,
                "paths": chunk,
                "include_current": true,
                "limit": 5_000,
            }),
        )
        .await?;
        remote_by_path.extend(plan.into_iter().map(|entry| (entry.path.clone(), entry)));
    }

    let mut files = Vec::with_capacity(selected.len());
    for depot_path in selected {
        let local_path = safe_workspace_target(&workspace.root, &depot_path)?;
        let tracked = state.files.get(&depot_path);
        let pending_action = pending
            .files
            .get(&depot_path)
            .map(|file| file.action.clone());
        let local_state = if !local_path.exists() {
            "missing"
        } else if pending_action.is_some() {
            "pending"
        } else if let Some(file) = tracked {
            if hash_file(&local_path).await? == file.local_hash {
                "clean"
            } else {
                "modified"
            }
        } else {
            "untracked"
        };
        let remote_revision = remote_by_path
            .get(&depot_path)
            .map(|entry| entry.revision_number);
        let remote_deleted = remote_by_path
            .get(&depot_path)
            .map(|entry| entry.deleted)
            .unwrap_or(false);
        let local_revision = tracked.map(|file| file.revision_number);
        let needs_sync = remote_revision
            .map(|remote| local_revision.unwrap_or_default() < remote)
            .unwrap_or(false);
        let lock = lock_by_path.get(&depot_path);
        let lock_state = lock.map(|lock| {
            if lock.workspace_id == workspace.workspace_id {
                "mine"
            } else {
                "other"
            }
        });
        files.push(serde_json::json!({
            "path": depot_path,
            "local_path": local_path,
            "local_state": local_state,
            "pending_action": pending_action,
            "local_revision": local_revision,
            "remote_revision": remote_revision,
            "remote_deleted": remote_deleted,
            "needs_sync": needs_sync,
            "lock_state": lock_state,
            "lock_reason": lock.and_then(|value| value.reason.clone()),
        }));
    }
    Ok(serde_json::json!({ "files": files }))
}

async fn integration_checkout(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
    reason: Option<String>,
) -> Result<serde_json::Value> {
    let (mut workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let local_path = safe_workspace_target(&workspace.root, &depot_path)?;
    if !local_path.is_file() {
        bail!("cannot check out missing file {}", local_path.display());
    }
    let state = load_state(&workspace_dir).await?;
    let changelist_id = ensure_integration_changelist(
        client,
        config,
        &mut workspace,
        &workspace_dir,
        "DCC checkout",
    )
    .await?;
    let (lock, lock_created) =
        acquire_integration_lock(client, config, &workspace, &depot_path, reason).await?;
    let action = if state.files.contains_key(&depot_path) {
        "edit"
    } else {
        "add"
    };
    let endpoint = if action == "add" {
        "/api/files/add"
    } else {
        "/api/files/edit"
    };
    let operation = authed_post_json::<serde_json::Value, _>(
        client,
        config,
        endpoint,
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "changelist_id": changelist_id,
            "path": depot_path,
        }),
    )
    .await;
    if let Err(error) = operation {
        if lock_created {
            let _ = authed_post_json::<serde_json::Value, _>(
                client,
                config,
                "/api/files/unlock",
                &serde_json::json!({
                    "workspace_id": workspace.workspace_id,
                    "path": depot_path,
                }),
            )
            .await;
        }
        return Err(error);
    }
    let mut pending = load_pending(&workspace_dir).await?;
    pending.files.insert(
        depot_path.clone(),
        PendingFile {
            action: action.to_string(),
        },
    );
    save_pending(&workspace_dir, &pending).await?;
    Ok(serde_json::json!({
        "path": depot_path,
        "action": action,
        "changelist_id": changelist_id,
        "lock": lock,
    }))
}

async fn integration_add(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
) -> Result<serde_json::Value> {
    let (mut workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let local_path = safe_workspace_target(&workspace.root, &depot_path)?;
    if !local_path.is_file() {
        bail!("cannot add missing file {}", local_path.display());
    }
    let state = load_state(&workspace_dir).await?;
    if state.files.contains_key(&depot_path) {
        bail!("{} is already tracked; check it out for edit", depot_path);
    }
    let rule: FileTypeRule = authed_post_json(
        client,
        config,
        "/api/filetypes/match",
        &serde_json::json!({ "path": depot_path }),
    )
    .await?;
    if rule.generated {
        bail!("{} is generated content and cannot be added", depot_path);
    }
    let changelist_id = ensure_integration_changelist(
        client,
        config,
        &mut workspace,
        &workspace_dir,
        "DCC asset add",
    )
    .await?;
    let lock = if rule.lock_required {
        let (lock, created) = acquire_integration_lock(
            client,
            config,
            &workspace,
            &depot_path,
            Some("New lock-required asset".to_string()),
        )
        .await?;
        Some((lock, created))
    } else {
        None
    };
    let operation = authed_post_json::<serde_json::Value, _>(
        client,
        config,
        "/api/files/add",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "changelist_id": changelist_id,
            "path": depot_path,
        }),
    )
    .await;
    if let Err(error) = operation {
        if lock.as_ref().map(|(_, created)| *created).unwrap_or(false) {
            let _ = authed_post_json::<serde_json::Value, _>(
                client,
                config,
                "/api/files/unlock",
                &serde_json::json!({
                    "workspace_id": workspace.workspace_id,
                    "path": depot_path,
                }),
            )
            .await;
        }
        return Err(error);
    }
    let mut pending = load_pending(&workspace_dir).await?;
    pending.files.insert(
        depot_path.clone(),
        PendingFile {
            action: "add".to_string(),
        },
    );
    save_pending(&workspace_dir, &pending).await?;
    Ok(serde_json::json!({
        "path": depot_path,
        "action": "add",
        "changelist_id": changelist_id,
        "lock": lock.map(|(lock, _)| lock),
    }))
}

async fn integration_delete(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
    reason: Option<String>,
) -> Result<serde_json::Value> {
    let (mut workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let changelist_id = ensure_integration_changelist(
        client,
        config,
        &mut workspace,
        &workspace_dir,
        "DCC asset delete",
    )
    .await?;
    let (lock, lock_created) =
        acquire_integration_lock(client, config, &workspace, &depot_path, reason).await?;
    let operation = authed_post_json::<serde_json::Value, _>(
        client,
        config,
        "/api/files/delete",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "changelist_id": changelist_id,
            "path": depot_path,
        }),
    )
    .await;
    if let Err(error) = operation {
        if lock_created {
            let _ = authed_post_json::<serde_json::Value, _>(
                client,
                config,
                "/api/files/unlock",
                &serde_json::json!({
                    "workspace_id": workspace.workspace_id,
                    "path": depot_path,
                }),
            )
            .await;
        }
        return Err(error);
    }
    let mut pending = load_pending(&workspace_dir).await?;
    pending.files.insert(
        depot_path.clone(),
        PendingFile {
            action: "delete".to_string(),
        },
    );
    save_pending(&workspace_dir, &pending).await?;
    Ok(serde_json::json!({
        "path": depot_path,
        "action": "delete",
        "changelist_id": changelist_id,
        "lock": lock,
    }))
}

async fn integration_lock(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
    reason: Option<String>,
) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let (lock, _) =
        acquire_integration_lock(client, config, &workspace, &depot_path, reason).await?;
    Ok(serde_json::to_value(lock)?)
}

async fn acquire_integration_lock(
    client: &reqwest::Client,
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
    depot_path: &str,
    reason: Option<String>,
) -> Result<(LockResponse, bool)> {
    let stream_id = resolve_workspace_stream_id(client, config, workspace).await?;
    let locks = list_locks_paged(client, config, Some(stream_id), None).await?;
    if let Some(lock) = locks.into_iter().find(|lock| {
        lock.stream_id == stream_id && lock.depot_path == depot_path && lock.state == "active"
    }) {
        if lock.workspace_id == workspace.workspace_id {
            return Ok((lock, false));
        }
        bail!("{} is already checked out in another workspace", depot_path);
    }
    let lock: LockResponse = authed_post_json(
        client,
        config,
        "/api/files/lock",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "path": depot_path,
            "reason": reason,
        }),
    )
    .await?;
    Ok((lock, true))
}

async fn resolve_workspace_stream_id(
    client: &reqwest::Client,
    config: &GlobalConfig,
    workspace: &WorkspaceConfig,
) -> Result<Uuid> {
    if let Some(id) = workspace.embedded_stream_id() {
        return Ok(id);
    }
    let depot_id = if let Ok(id) = Uuid::parse_str(&workspace.depot) {
        id
    } else {
        let depots: Vec<DepotResponse> = authed_get_json(client, config, "/api/depots").await?;
        depots
            .into_iter()
            .find(|depot| depot.name == workspace.depot)
            .map(|depot| depot.id)
            .ok_or_else(|| anyhow!("workspace depot no longer exists: {}", workspace.depot))?
    };
    let streams: Vec<StreamResponse> = authed_get_json(client, config, "/api/streams").await?;
    streams
        .into_iter()
        .find(|stream| stream.depot_id == depot_id && stream.name == workspace.stream)
        .map(|stream| stream.id)
        .ok_or_else(|| anyhow!("workspace stream no longer exists: {}", workspace.stream))
}

async fn integration_unlock(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let lock: LockResponse = authed_post_json(
        client,
        config,
        "/api/files/unlock",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "path": depot_path,
        }),
    )
    .await?;
    Ok(serde_json::to_value(lock)?)
}

async fn integration_revert(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let mut pending = load_pending(&workspace_dir).await?;
    let previous_action = pending.files.remove(&depot_path).map(|file| file.action);
    let _: serde_json::Value = authed_post_json(
        client,
        config,
        "/api/files/revert",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "changelist_id": workspace.active_changelist_id,
            "path": depot_path,
        }),
    )
    .await?;
    save_pending(&workspace_dir, &pending).await?;

    let locks = list_locks_paged(client, config, None, Some(workspace.workspace_id)).await?;
    let held_here = locks.iter().any(|lock| {
        lock.workspace_id == workspace.workspace_id
            && lock.depot_path == depot_path
            && lock.state == "active"
    });
    if held_here {
        let _: LockResponse = authed_post_json(
            client,
            config,
            "/api/files/unlock",
            &serde_json::json!({
                "workspace_id": workspace.workspace_id,
                "path": depot_path,
            }),
        )
        .await?;
    }
    Ok(serde_json::json!({
        "path": depot_path,
        "reverted_action": previous_action,
        "local_file_preserved": true,
        "lock_released": held_here,
    }))
}

async fn integration_history(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
) -> Result<serde_json::Value> {
    let (workspace, _) = load_workspace().await?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let url = format!(
        "/api/files/history?workspace_id={}&path={}",
        workspace.workspace_id,
        urlencoding::encode(&depot_path)
    );
    let entries: Vec<HistoryEntry> = authed_get_json(client, config, &url).await?;
    Ok(serde_json::json!({ "path": depot_path, "revisions": entries }))
}

async fn integration_preview(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
    image: PathBuf,
) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let state = load_state(&workspace_dir).await?;
    let revision_number = state
        .files
        .get(&depot_path)
        .map(|file| file.revision_number)
        .ok_or_else(|| anyhow!("cannot upload a preview before the asset is submitted"))?;
    let content_type = match image
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => bail!("preview image must be PNG, JPEG, or WebP"),
    };
    let bytes = fs::read(&image)
        .await
        .with_context(|| format!("failed to read preview image {}", image.display()))?;
    let url = format!(
        "/api/files/preview?workspace_id={}&path={}&revision_number={}",
        workspace.workspace_id,
        urlencoding::encode(&depot_path),
        revision_number
    );
    let response = authed_request(
        config,
        client
            .post(api_url(config, &url))
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes),
    )?
    .send()
    .await?;
    parse_response(response).await
}

async fn integration_review_proxy(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: PathBuf,
    media: PathBuf,
) -> Result<serde_json::Value> {
    let (workspace, workspace_dir) = load_workspace().await?;
    let _operation_lock = lock_workspace_operation(&workspace_dir)?;
    let depot_path = depot_path_for_input(&workspace.root, &path)?;
    let state = load_state(&workspace_dir).await?;
    let revision_number = state
        .files
        .get(&depot_path)
        .map(|file| file.revision_number)
        .ok_or_else(|| anyhow!("cannot upload a review proxy before the asset is submitted"))?;
    let content_type = match media
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "fbx" => "application/vnd.autodesk.fbx",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => bail!("review proxy must be GLB, glTF, FBX, MP4, or WebM"),
    };
    let bytes = fs::read(&media)
        .await
        .with_context(|| format!("failed to read review proxy {}", media.display()))?;
    let url = format!(
        "/api/reviews/proxy?workspace_id={}&path={}&revision_number={}",
        workspace.workspace_id,
        urlencoding::encode(&depot_path),
        revision_number
    );
    let response = authed_request(
        config,
        client
            .post(api_url(config, &url))
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes),
    )?
    .send()
    .await?;
    parse_response(response).await
}

async fn integration_validate(
    client: &reqwest::Client,
    config: &GlobalConfig,
    paths: Vec<PathBuf>,
    adapter: Option<String>,
) -> Result<serde_json::Value> {
    let (workspace, _) = load_workspace().await?;
    let paths = if paths.is_empty() {
        workspace_files(&workspace.root)?
    } else {
        paths
            .iter()
            .map(|path| depot_path_for_input(&workspace.root, path))
            .collect::<Result<Vec<_>>>()?
    };
    let mut files = Vec::with_capacity(paths.len());
    let mut core_paths = Vec::with_capacity(paths.len());
    for path in paths {
        let file = adapter_file_input(&workspace.root, path).await?;
        core_paths.push(serde_json::json!({
            "path": file.path,
            "size_bytes": file.size_bytes,
        }));
        files.push(file);
    }
    let adapter_validation: AdapterValidationResponse = authed_post_json(
        client,
        config,
        "/api/adapters/validate",
        &serde_json::json!({ "adapter_name": adapter, "files": files }),
    )
    .await?;
    let core_validation: ValidationResponse = authed_post_json(
        client,
        config,
        "/api/validate",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "paths": core_paths,
        }),
    )
    .await?;
    Ok(serde_json::json!({
        "adapter": adapter_validation,
        "core": core_validation,
    }))
}

async fn ensure_integration_changelist(
    client: &reqwest::Client,
    config: &GlobalConfig,
    workspace: &mut WorkspaceConfig,
    workspace_dir: &Path,
    description: &str,
) -> Result<Uuid> {
    if let Some(id) = workspace.active_changelist_id {
        return Ok(id);
    }
    let response: ChangelistResponse = authed_post_json(
        client,
        config,
        "/api/changelists",
        &serde_json::json!({
            "workspace_id": workspace.workspace_id,
            "description": description,
        }),
    )
    .await?;
    workspace.active_changelist_id = Some(response.id);
    save_workspace(workspace_dir, workspace).await?;
    Ok(response.id)
}

async fn authed_get_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: &str,
) -> Result<T> {
    let response = authed_request(config, client.get(api_url(config, path)))?
        .send()
        .await?;
    parse_response(response).await
}

async fn list_locks_paged(
    client: &reqwest::Client,
    config: &GlobalConfig,
    stream_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
) -> Result<Vec<LockResponse>> {
    let mut locks = Vec::new();
    let mut before_created_at: Option<String> = None;
    let mut before_id: Option<Uuid> = None;
    loop {
        let mut query = vec![("limit", "1000".to_string())];
        if let Some(value) = stream_id {
            query.push(("stream_id", value.to_string()));
        }
        if let Some(value) = workspace_id {
            query.push(("workspace_id", value.to_string()));
        }
        if let (Some(created_at), Some(id)) = (&before_created_at, before_id) {
            query.push(("before_created_at", created_at.clone()));
            query.push(("before_id", id.to_string()));
        }
        let encoded = query
            .into_iter()
            .map(|(key, value)| format!("{key}={}", urlencoding::encode(&value)))
            .collect::<Vec<_>>()
            .join("&");
        let page: LockPageResponse =
            authed_get_json(client, config, &format!("/api/locks/page?{encoded}")).await?;
        locks.extend(page.items);
        let next_created_at = page.next_before_created_at;
        let next_id = page.next_before_id;
        if next_created_at.is_none() || next_id.is_none() {
            return Ok(locks);
        }
        if next_created_at == before_created_at && next_id == before_id {
            bail!("server returned a repeated lock-page cursor");
        }
        before_created_at = next_created_at;
        before_id = next_id;
    }
}

async fn authed_post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: &str,
    body: &B,
) -> Result<T> {
    let response = authed_request(config, client.post(api_url(config, path)))?
        .json(body)
        .send()
        .await?;
    parse_response(response).await
}

async fn authed_delete_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: &str,
) -> Result<T> {
    let response = authed_request(config, client.delete(api_url(config, path)))?
        .send()
        .await?;
    parse_response(response).await
}

async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
    client: &reqwest::Client,
    config: &GlobalConfig,
    path: &str,
    body: &B,
) -> Result<T> {
    let response = client.post(api_url(config, path)).json(body).send().await?;
    parse_response(response).await
}

fn authed_request(
    config: &GlobalConfig,
    request: reqwest::RequestBuilder,
) -> Result<reqwest::RequestBuilder> {
    let token = config.token.as_deref().ok_or_else(|| {
        anyhow!("not logged in; run `oad login <username> --password <password>`")
    })?;
    Ok(request.bearer_auth(token))
}

async fn parse_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    let response = ensure_success(response).await?;
    Ok(response.json::<T>().await?)
}

async fn ensure_success(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    bail!("request failed with {status}: {text}");
}

fn api_url(config: &GlobalConfig, path: &str) -> String {
    format!("{}{}", config.server_url.trim_end_matches('/'), path)
}

async fn load_global_config() -> Result<GlobalConfig> {
    let path = global_config_path()?;
    if !path.exists() {
        return Ok(GlobalConfig {
            server_url: "http://127.0.0.1:8080".to_string(),
            ..Default::default()
        });
    }
    read_json(&path).await
}

async fn save_global_config(config: &GlobalConfig) -> Result<()> {
    let path = global_config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    write_json(&path, config).await
}

fn global_config_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| anyhow!("could not determine home directory"))?
        .join(".oad")
        .join("config.json"))
}

async fn load_workspace() -> Result<(WorkspaceConfig, PathBuf)> {
    let mut dir = std::env::current_dir()?;
    loop {
        let oad_dir = dir.join(".oad");
        let config_path = oad_dir.join("workspace.json");
        if config_path.exists() {
            let mut workspace: WorkspaceConfig = read_json(&config_path).await?;
            workspace.root = workspace.root.canonicalize().unwrap_or(workspace.root);
            return Ok((workspace, oad_dir));
        }
        if !dir.pop() {
            bail!("not inside an OpenAsset Depot workspace");
        }
    }
}

async fn save_workspace(oad_dir: &Path, workspace: &WorkspaceConfig) -> Result<()> {
    write_json(&oad_dir.join("workspace.json"), workspace).await
}

async fn load_state(oad_dir: &Path) -> Result<WorkspaceState> {
    read_json_or_default(&oad_dir.join("state.json")).await
}

async fn save_state(oad_dir: &Path, state: &WorkspaceState) -> Result<()> {
    write_json(&oad_dir.join("state.json"), state).await
}

async fn load_pending(oad_dir: &Path) -> Result<PendingState> {
    read_json_or_default(&oad_dir.join("pending.json")).await
}

async fn save_pending(oad_dir: &Path, state: &PendingState) -> Result<()> {
    write_json(&oad_dir.join("pending.json"), state).await
}

async fn read_json_or_default<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    read_json(path).await
}

async fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let data = fs::read(path).await?;
    Ok(serde_json::from_slice(&data)?)
}

async fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let data = serde_json::to_vec_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("invalid metadata file path: {}", path.display()))?;
    let temp_path = path.with_file_name(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let result: Result<()> = async {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temp_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        fs::rename(&temp_path, path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temp_path).await;
    }
    result
}

struct WorkspaceOperationLock {
    file: std::fs::File,
}

impl Drop for WorkspaceOperationLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn lock_workspace_operation(oad_dir: &Path) -> Result<WorkspaceOperationLock> {
    let path = oad_dir.join("operation.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("failed to open workspace lock {}", path.display()))?;
    file.try_lock_exclusive().with_context(|| {
        format!(
            "another OpenAsset operation is already running for this workspace ({})",
            path.display()
        )
    })?;
    Ok(WorkspaceOperationLock { file })
}

fn depot_path_for_input(root: &Path, input: &Path) -> Result<String> {
    let path = if input.is_absolute() {
        input.to_path_buf()
    } else {
        std::env::current_dir()?.join(input)
    };
    let canonical = path.canonicalize().unwrap_or(path);
    depot_path_for(root, &canonical)
}

fn depot_path_for(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).with_context(|| {
        format!(
            "{} is outside workspace root {}",
            path.display(),
            root.display()
        )
    })?;
    let value = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    if value.is_empty() || value.contains("..") || value.contains('\\') {
        bail!("invalid depot path: {value}");
    }
    Ok(value)
}

fn safe_workspace_target(root: &Path, depot_path: &str) -> Result<PathBuf> {
    if depot_path.is_empty()
        || depot_path.contains('\\')
        || Path::new(depot_path).is_absolute()
        || !Path::new(depot_path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("invalid depot path: {depot_path}");
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let target = root.join(depot_path);
    let mut existing_parent = target.parent();
    while let Some(parent) = existing_parent {
        if parent.exists() {
            let canonical_parent = parent.canonicalize().with_context(|| {
                format!("failed to resolve workspace path {}", parent.display())
            })?;
            if !canonical_parent.starts_with(&root) {
                bail!(
                    "workspace path escapes through a symbolic link: {}",
                    target.display()
                );
            }
            break;
        }
        existing_parent = parent.parent();
    }
    Ok(target)
}

async fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn workspace_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .path()
            .components()
            .any(|component| component.as_os_str() == ".oad")
        {
            continue;
        }
        files.push(depot_path_for(root, entry.path())?);
    }
    Ok(files)
}

async fn adapter_file_input(root: &Path, depot_path: String) -> Result<AdapterFileInput> {
    let local_path = root.join(&depot_path);
    let metadata = fs::metadata(&local_path).await.ok();
    let size_bytes = metadata.as_ref().map(|meta| meta.len());
    let content = if should_read_adapter_text(&depot_path, size_bytes) {
        Some(fs::read_to_string(&local_path).await.with_context(|| {
            format!(
                "failed to read text-scannable adapter file {}",
                local_path.display()
            )
        })?)
    } else {
        None
    };
    Ok(AdapterFileInput {
        path: depot_path,
        content,
        size_bytes,
    })
}

fn should_read_adapter_text(path: &str, size_bytes: Option<u64>) -> bool {
    const MAX_TEXT_SCAN_BYTES: u64 = 1024 * 1024;
    matches!(size_bytes, Some(size) if size <= MAX_TEXT_SCAN_BYTES)
        && (path.ends_with(".ma") || path.ends_with(".nk"))
}

fn is_unity_asset_needing_meta(path: &str) -> bool {
    !path.ends_with(".meta")
        && (path.ends_with(".unity")
            || path.ends_with(".prefab")
            || path.ends_with(".asset")
            || path.ends_with(".mat")
            || path.ends_with(".controller"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_workspace_target_rejects_traversal_and_absolute_paths() {
        let temp = tempfile::tempdir().unwrap();
        assert!(safe_workspace_target(temp.path(), "../outside.blend").is_err());
        assert!(safe_workspace_target(temp.path(), "/outside.blend").is_err());
        assert!(safe_workspace_target(temp.path(), "Scenes\\outside.blend").is_err());
        assert!(safe_workspace_target(temp.path(), "Scenes/shot..v2.blend").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn safe_workspace_target_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), workspace.path().join("linked")).unwrap();
        assert!(safe_workspace_target(workspace.path(), "linked/scene.blend").is_err());
    }

    #[tokio::test]
    async fn metadata_writes_replace_atomically_without_leaving_temp_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        write_json(&path, &serde_json::json!({ "version": 1 }))
            .await
            .unwrap();
        write_json(&path, &serde_json::json!({ "version": 2 }))
            .await
            .unwrap();

        let value: serde_json::Value = read_json(&path).await.unwrap();
        assert_eq!(value["version"], 2);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let entries = std::fs::read_dir(temp.path()).unwrap().count();
        assert_eq!(entries, 1);
    }

    #[test]
    fn workspace_operation_lock_is_exclusive() {
        let temp = tempfile::tempdir().unwrap();
        let first = lock_workspace_operation(temp.path()).unwrap();
        assert!(lock_workspace_operation(temp.path()).is_err());
        drop(first);
        assert!(lock_workspace_operation(temp.path()).is_ok());
    }

    #[test]
    fn integration_protocol_v2_is_opt_in_and_v1_remains_default() {
        let v2 =
            Cli::try_parse_from(["oad", "integration", "--protocol-version", "2", "sync"]).unwrap();
        assert!(matches!(
            v2.command,
            Command::Integration {
                protocol_version: 2,
                command: IntegrationCommand::Sync,
            }
        ));

        let v1 = Cli::try_parse_from(["oad", "integration", "sync"]).unwrap();
        assert!(matches!(
            v1.command,
            Command::Integration {
                protocol_version: 1,
                command: IntegrationCommand::Sync,
            }
        ));
    }

    #[test]
    fn integration_accepts_portable_review_proxy_media() {
        let cli = Cli::try_parse_from([
            "oad",
            "integration",
            "review-proxy",
            "Scenes/Shot.blend",
            "--media",
            "/tmp/Shot.glb",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Integration {
                command: IntegrationCommand::ReviewProxy { .. },
                ..
            }
        ));
    }

    #[test]
    fn saved_server_is_used_until_an_explicit_override_is_provided() {
        let saved = Cli::try_parse_from(["oad", "workspace", "list"]).unwrap();
        assert_eq!(saved.server, None);

        let overridden = Cli::try_parse_from([
            "oad",
            "--server",
            "https://asset.example.com",
            "workspace",
            "list",
        ])
        .unwrap();
        assert_eq!(
            overridden.server.as_deref(),
            Some("https://asset.example.com")
        );
    }
}
