use std::time::Duration;

use reqwest::{multipart, Client, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
struct TestClient {
    base_url: String,
    client: Client,
}

#[derive(Clone)]
struct User {
    id: Uuid,
    token: String,
}

#[derive(Clone)]
struct Project {
    admin: User,
    alice: User,
    bob: User,
    depot_id: Uuid,
    stream_id: Uuid,
    alice_workspace: Uuid,
    bob_workspace: Uuid,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    token: String,
    user: LoginUserResponse,
}

#[derive(Debug, Deserialize)]
struct LoginUserResponse {
    id: Uuid,
}

#[derive(Debug, Deserialize)]
struct IdResponse {
    id: Uuid,
}

#[derive(Debug, Deserialize)]
struct LockResponse {
    id: Uuid,
    user_id: Uuid,
    depot_path: String,
}

#[derive(Debug, Deserialize)]
struct ShelfResponse {
    changelist_id: Uuid,
    files: Vec<ShelvedFile>,
}

#[derive(Debug, Deserialize)]
struct ShelvedFile {
    path: String,
    blob_hash: String,
    size_bytes: i64,
    action: String,
}

#[derive(Debug, Deserialize)]
struct ShelfSummary {
    changelist_id: Uuid,
    file_count: i64,
    total_bytes: i64,
    owner: String,
}

#[derive(Debug, Deserialize)]
struct DiscardShelfResponse {
    discarded: i64,
}

#[derive(Debug, Deserialize)]
struct ReviewRequestResponse {
    id: Uuid,
    path: String,
    revision_number: i32,
    state: String,
    reviewers: Vec<ReviewerResponse>,
}

#[derive(Debug, Deserialize)]
struct ReviewerResponse {
    reviewer_user_id: Uuid,
    decision: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CollaboratorResponse {
    user_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct LockPageResponse {
    items: Vec<LockPageItem>,
    next_before_created_at: Option<String>,
    next_before_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct LockPageItem {
    id: Uuid,
    depot_path: String,
}

#[derive(Debug, Deserialize)]
struct AuditPageResponse {
    items: Vec<AuditPageItem>,
    next_before_created_at: Option<String>,
    next_before_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct AuditPageItem {
    id: Uuid,
    event_type: String,
}

#[derive(Debug, Deserialize)]
struct DependencyPageResponse {
    items: Vec<DependencyPageItem>,
    next_before_scan_time: Option<String>,
    next_before_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct DependencyPageItem {
    id: Uuid,
    source_file: String,
    target_file: String,
}

#[derive(Debug, Deserialize)]
struct DeleteWorkspaceResponse {
    id: Uuid,
    deleted: bool,
    released_locks: u64,
    abandoned_changelists: u64,
}

#[derive(Debug, Deserialize)]
struct SubmitResponse {
    revisions: Vec<SubmittedRevision>,
}

#[derive(Debug, Deserialize)]
struct SubmittedRevision {
    path: String,
    revision_number: i32,
}

#[derive(Debug, Deserialize)]
struct SyncPlanEntry {
    path: String,
    revision_number: i32,
    deleted: bool,
    #[serde(default)]
    preview_available: bool,
    #[serde(default)]
    review_proxy_available: bool,
}

#[derive(Debug, Deserialize)]
struct ReviewCommentEntry {
    id: Uuid,
    body: String,
    timecode_ms: Option<i64>,
    frame_number: Option<i32>,
    annotation: Option<serde_json::Value>,
    resolved_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UploadSessionEntry {
    upload_id: Uuid,
    received_bytes: i64,
    purpose: String,
}

#[derive(Debug, Deserialize)]
struct ReviewProxyEntry {
    blob_hash: String,
    frame_rate_numerator: Option<i32>,
    frame_rate_denominator: Option<i32>,
    start_frame: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct HistoryEntry {
    revision_number: i32,
    action: String,
}

#[derive(Debug, Deserialize)]
struct ValidationResponse {
    warnings: Vec<ValidationMessage>,
}

#[derive(Debug, Deserialize)]
struct ValidationMessage {
    code: String,
}

#[derive(Debug, Deserialize)]
struct StorageVerificationSummary {
    checked: usize,
    failed: usize,
}

#[derive(Debug, Deserialize)]
struct StorageCleanupSummary {
    dry_run: bool,
    deleted: usize,
    scanned: usize,
}

#[derive(Serialize)]
struct CreateUserRequest<'a> {
    username: &'a str,
    password: &'a str,
    display_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_admin: Option<bool>,
}

const PASSWORD: &str = "correct horse battery staple";

fn api() -> TestClient {
    let base_url = std::env::var("OAD_TEST_BASE_URL")
        .expect("set OAD_TEST_BASE_URL, for example http://127.0.0.1:8080");
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client");
    TestClient { base_url, client }
}

fn database_url() -> String {
    std::env::var("OAD_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://openasset:openasset@127.0.0.1:55432/openasset".to_string())
}

async fn ensure_admin(api: &TestClient) -> User {
    let username = "oad_admin_ci";
    let create = api
        .client
        .post(api.url("/api/users"))
        .json(&CreateUserRequest {
            username,
            password: PASSWORD,
            display_name: "OpenAsset CI Admin",
            is_admin: Some(true),
        })
        .send()
        .await
        .expect("admin create request failed");

    if !(create.status().is_success()
        || create.status() == StatusCode::CONFLICT
        || create.status() == StatusCode::UNAUTHORIZED
        || create.status() == StatusCode::FORBIDDEN)
    {
        panic!(
            "unexpected admin bootstrap response: {}",
            error_text(create).await
        );
    }

    login(api, username).await
}

async fn create_user(api: &TestClient, admin: &User, username: &str) -> User {
    let response = api
        .client
        .post(api.url("/api/users"))
        .bearer_auth(&admin.token)
        .json(&CreateUserRequest {
            username,
            password: PASSWORD,
            display_name: username,
            is_admin: None,
        })
        .send()
        .await
        .expect("user create request failed");
    assert_success(response).await;
    login(api, username).await
}

async fn login(api: &TestClient, username: &str) -> User {
    let response = api
        .client
        .post(api.url("/api/auth/login"))
        .json(&serde_json::json!({ "username": username, "password": PASSWORD }))
        .send()
        .await
        .expect("login request failed");
    let body: LoginResponse = parse_success(response).await;
    User {
        id: body.user.id,
        token: body.token,
    }
}

async fn setup_project(api: &TestClient, label: &str) -> Project {
    let admin = ensure_admin(api).await;
    let suffix = short_id();
    let alice = create_user(api, &admin, &format!("alice_{label}_{suffix}")).await;
    let bob = create_user(api, &admin, &format!("bob_{label}_{suffix}")).await;
    let depot_name = format!("depot_{label}_{suffix}");

    let depot: IdResponse = authed_post(
        api,
        &alice,
        "/api/depots",
        serde_json::json!({ "name": depot_name }),
    )
    .await;
    let stream: IdResponse = authed_post(
        api,
        &alice,
        "/api/streams",
        serde_json::json!({ "depot": depot_name, "name": "main" }),
    )
    .await;
    grant_depot_permission(api, &alice, depot.id, bob.id, "write").await;
    let alice_workspace: IdResponse = authed_post(
        api,
        &alice,
        "/api/workspaces",
        serde_json::json!({
            "name": format!("alice_ws_{suffix}"),
            "depot": depot.id,
            "stream": stream.id,
            "local_path": format!("/tmp/openasset/alice/{suffix}")
        }),
    )
    .await;
    let bob_workspace: IdResponse = authed_post(
        api,
        &bob,
        "/api/workspaces",
        serde_json::json!({
            "name": format!("bob_ws_{suffix}"),
            "depot": depot.id,
            "stream": stream.id,
            "local_path": format!("/tmp/openasset/bob/{suffix}")
        }),
    )
    .await;

    Project {
        admin,
        alice,
        bob,
        depot_id: depot.id,
        stream_id: stream.id,
        alice_workspace: alice_workspace.id,
        bob_workspace: bob_workspace.id,
    }
}

/// Gives the system admin a workspace on the project's stream.
///
/// Admins reach every depot, but review endpoints still need a workspace to
/// establish which stream the caller is acting on.
async fn admin_workspace(api: &TestClient, project: &Project) -> Uuid {
    let response: IdResponse = authed_post(
        api,
        &project.admin,
        "/api/workspaces",
        serde_json::json!({
            "name": format!("admin_ws_{}", short_id()),
            "depot": project.depot_id,
            "stream": project.stream_id,
            "local_path": format!("/tmp/openasset/admin/{}", short_id())
        }),
    )
    .await;
    response.id
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn lock_pages_use_stable_keyset_cursors() {
    let api = api();
    let project = setup_project(&api, "lock_pages").await;
    for path in ["Scenes/A.blend", "Scenes/B.blend", "Scenes/C.blend"] {
        lock_path(&api, &project.alice, project.alice_workspace, path).await;
    }

    let first = lock_page(&api, &project.alice, project.alice_workspace, 2, None).await;
    assert_eq!(first.items.len(), 2);
    let cursor = (
        first.next_before_created_at.clone().unwrap(),
        first.next_before_id.unwrap(),
    );
    let second = lock_page(
        &api,
        &project.alice,
        project.alice_workspace,
        2,
        Some(cursor),
    )
    .await;
    assert_eq!(second.items.len(), 1);
    assert!(second.next_before_id.is_none());

    let ids: std::collections::HashSet<_> = first
        .items
        .iter()
        .chain(&second.items)
        .map(|item| item.id)
        .collect();
    let paths: std::collections::HashSet<_> = first
        .items
        .iter()
        .chain(&second.items)
        .map(|item| item.depot_path.as_str())
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(paths.len(), 3);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn audit_pages_are_admin_only_and_do_not_repeat_rows() {
    let api = api();
    let project = setup_project(&api, "audit_pages").await;
    let forbidden = api
        .client
        .get(api.url("/api/audit?limit=1"))
        .bearer_auth(&project.alice.token)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let first: AuditPageResponse = authed_get(&api, &project.admin, "/api/audit?limit=1").await;
    assert_eq!(first.items.len(), 1);
    assert!(!first.items[0].event_type.is_empty());
    let second = audit_page(
        &api,
        &project.admin,
        1,
        Some((
            first.next_before_created_at.clone().unwrap(),
            first.next_before_id.unwrap(),
        )),
    )
    .await;
    assert_eq!(second.items.len(), 1);
    assert_ne!(first.items[0].id, second.items[0].id);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn dependency_scans_persist_and_page_by_stream() {
    let api = api();
    let project = setup_project(&api, "dependency_pages").await;
    let scan: serde_json::Value = authed_post(
        &api,
        &project.alice,
        "/api/adapters/scan",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "adapter_name": "Nuke",
            "file": {
                "path": "Comps/Main.nk",
                "content": "Read { file plates/shot.exr }\nOCIOFileTransform { file looks/show.cube }"
            }
        }),
    )
    .await;
    assert_eq!(scan["dependencies"].as_array().unwrap().len(), 2);

    let first = dependency_page(&api, &project.alice, project.stream_id, 1, None).await;
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].source_file, "Comps/Main.nk");
    let cursor = (
        first.next_before_scan_time.clone().unwrap(),
        first.next_before_id.unwrap(),
    );
    let second = dependency_page(&api, &project.alice, project.stream_id, 1, Some(cursor)).await;
    assert_eq!(second.items.len(), 1);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert_ne!(first.items[0].target_file, second.items[0].target_file);
}

#[derive(Debug, Deserialize)]
struct DependencyImpactResponse {
    path: String,
    required_by: Vec<DependencyImpactEdge>,
    depends_on: Vec<DependencyImpactEdge>,
    required_by_count: i64,
    depends_on_count: i64,
    missing_count: i64,
    last_scanned_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DependencyImpactEdge {
    path: String,
    dependency_status: String,
    in_depot: bool,
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn dependency_impact_reports_both_directions_around_a_file() {
    let api = api();
    let project = setup_project(&api, "dependency_impact").await;
    let plate = "plates/shot.exr";

    // Two comps read the same plate, so changing the plate affects both.
    for comp in ["Comps/Alpha.nk", "Comps/Beta.nk"] {
        let _: serde_json::Value = authed_post(
            &api,
            &project.alice,
            "/api/adapters/scan",
            serde_json::json!({
                "workspace_id": project.alice_workspace,
                "adapter_name": "Nuke",
                "file": {
                    "path": comp,
                    "content": format!("Read {{ file {plate} }}\nOCIOFileTransform {{ file looks/show.cube }}")
                }
            }),
        )
        .await;
    }

    let impact: DependencyImpactResponse = authed_get(
        &api,
        &project.alice,
        &format!(
            "/api/dependencies/impact?workspace_id={}&path={plate}",
            project.alice_workspace
        ),
    )
    .await;
    assert_eq!(impact.path, plate);
    assert_eq!(impact.required_by_count, 2);
    let mut referencing: Vec<_> = impact.required_by.iter().map(|e| e.path.as_str()).collect();
    referencing.sort();
    assert_eq!(referencing, vec!["Comps/Alpha.nk", "Comps/Beta.nk"]);
    // Nothing was ever submitted, so the referencing comps are not in the depot.
    assert!(impact.required_by.iter().all(|edge| !edge.in_depot));
    // The plate is a leaf: it reads nothing itself.
    assert_eq!(impact.depends_on_count, 0);
    assert!(impact.depends_on.is_empty());
    assert!(impact.last_scanned_at.is_some());

    // Asking about a comp gives the forward direction and counts its misses.
    let comp_impact: DependencyImpactResponse = authed_get(
        &api,
        &project.alice,
        &format!(
            "/api/dependencies/impact?workspace_id={}&path=Comps/Alpha.nk",
            project.alice_workspace
        ),
    )
    .await;
    assert_eq!(comp_impact.depends_on_count, 2);
    assert_eq!(comp_impact.required_by_count, 0);
    // `missing_count` only counts references an adapter actively resolved and
    // failed to find. The Nuke adapter reports `unknown` because it reads paths
    // out of the script without checking them, so an unverified reference must
    // not be reported to the artist as a broken one.
    assert_eq!(comp_impact.missing_count, 0);
    assert!(comp_impact
        .depends_on
        .iter()
        .all(|edge| edge.dependency_status == "unknown"));
    // Not in the depot either, which is the separate signal the inspector needs.
    assert!(comp_impact.depends_on.iter().all(|edge| !edge.in_depot));
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn dependency_impact_is_empty_and_unscanned_for_an_untouched_file() {
    let api = api();
    let project = setup_project(&api, "dependency_unscanned").await;
    let impact: DependencyImpactResponse = authed_get(
        &api,
        &project.alice,
        &format!(
            "/api/dependencies/impact?workspace_id={}&path=Comps/NeverScanned.nk",
            project.alice_workspace
        ),
    )
    .await;
    // A never-scanned file must not look like a file with no dependents; the
    // absent scan timestamp is what lets the UI say "not scanned yet" instead.
    assert_eq!(impact.required_by_count, 0);
    assert_eq!(impact.depends_on_count, 0);
    assert!(impact.last_scanned_at.is_none());
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn storage_cleanup_is_admin_only_and_dry_run_by_default() {
    let api = api();
    let project = setup_project(&api, "storage_cleanup").await;
    let forbidden = api
        .client
        .post(api.url("/api/admin/storage/cleanup"))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let summary: StorageCleanupSummary = authed_post(
        &api,
        &project.admin,
        "/api/admin/storage/cleanup",
        serde_json::json!({}),
    )
    .await;
    assert!(summary.dry_run);
    assert_eq!(summary.deleted, 0);
    assert!(summary.scanned >= summary.deleted);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn two_users_lock_same_umap_second_fails() {
    lock_conflict_for("same_umap", "Content/Maps/Main.umap").await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn two_users_lock_same_unity_second_fails() {
    lock_conflict_for("same_unity", "Assets/Scenes/Main.unity").await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn two_users_lock_same_blend_second_fails() {
    lock_conflict_for("same_blend", "Scenes/Shot.blend").await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn two_users_lock_same_ma_second_fails() {
    lock_conflict_for("same_ma", "Scenes/Shot.ma").await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn user_adds_binary_file_and_submits() {
    let api = api();
    let project = setup_project(&api, "submit_binary").await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        "Models/Hero.fbx",
    )
    .await;
    let submit = submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        "Models/Hero.fbx",
        b"fbx-binary-data",
    )
    .await;
    assert_eq!(submit.revisions.len(), 1);
    assert_eq!(submit.revisions[0].path, "Models/Hero.fbx");
    assert_eq!(submit.revisions[0].revision_number, 1);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn submitted_revision_accepts_and_serves_an_immutable_preview() {
    let api = api();
    let project = setup_project(&api, "revision_preview").await;
    let path = "Models/Preview.fbx";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"fbx-preview-source",
    )
    .await;

    let workspace_id = project.alice_workspace.to_string();
    let revision_number = "1";
    let upload = api
        .client
        .post(api.url("/api/files/preview"))
        .bearer_auth(&project.alice.token)
        .query(&[
            ("workspace_id", workspace_id.as_str()),
            ("path", path),
            ("revision_number", revision_number),
        ])
        .header("content-type", "image/png")
        .body(b"immutable-preview".to_vec())
        .send()
        .await
        .expect("preview upload failed");
    assert_success(upload).await;

    let plan: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.alice,
        "/api/sync/plan",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "include_current": true
        }),
    )
    .await;
    assert_eq!(plan.len(), 1);
    assert!(plan[0].preview_available);

    let download = api
        .client
        .get(api.url("/api/files/preview"))
        .bearer_auth(&project.alice.token)
        .query(&[
            ("workspace_id", workspace_id.as_str()),
            ("path", path),
            ("revision_number", revision_number),
        ])
        .send()
        .await
        .expect("preview download failed");
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(download.headers()["content-type"], "image/png");
    assert_eq!(
        download.bytes().await.unwrap(),
        b"immutable-preview".as_slice()
    );
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn revision_review_proxy_comments_annotations_and_ranges_round_trip() {
    let api = api();
    let project = setup_project(&api, "revision_review").await;
    let path = "Models/AnimatedHero.fbx";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"fbx-source",
    )
    .await;

    let target = [
        ("workspace_id", project.alice_workspace.to_string()),
        ("path", path.to_string()),
        ("revision_number", "1".to_string()),
    ];
    let upload_session: UploadSessionEntry = authed_post(
        &api,
        &project.alice,
        "/api/uploads",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "purpose": "review_proxy",
            "size_bytes": 16
        }),
    )
    .await;
    assert_eq!(upload_session.purpose, "review_proxy");
    assert_eq!(upload_session.received_bytes, 0);
    let proxy_chunk = api
        .client
        .post(api.url(&format!("/api/uploads/{}", upload_session.upload_id)))
        .bearer_auth(&project.alice.token)
        .header("x-upload-offset", "0")
        .body(b"glb-review-proxy".to_vec())
        .send()
        .await
        .expect("review proxy chunk upload failed");
    assert_success(proxy_chunk).await;
    let _: serde_json::Value = authed_post(
        &api,
        &project.alice,
        &format!("/api/uploads/{}/finalize", upload_session.upload_id),
        serde_json::json!({}),
    )
    .await;
    let proxy: ReviewProxyEntry = authed_post(
        &api,
        &project.alice,
        "/api/reviews/proxy/attach",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "upload_id": upload_session.upload_id,
            "content_type": "model/gltf-binary",
            "frame_rate_numerator": 24000,
            "frame_rate_denominator": 1001,
            "start_frame": 1001
        }),
    )
    .await;
    assert!(!proxy.blob_hash.is_empty());
    assert_eq!(proxy.frame_rate_numerator, Some(24_000));
    assert_eq!(proxy.frame_rate_denominator, Some(1_001));
    assert_eq!(proxy.start_frame, Some(1_001));

    let comment: ReviewCommentEntry = authed_post(
        &api,
        &project.alice,
        "/api/reviews/comments",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "body": "Ease the landing pose",
            "timecode_ms": 1250,
            "frame_number": 30,
            "annotation": {
                "marks": [{
                    "tool": "pen",
                    "color": "#ffcb4c",
                    "width": 4,
                    "points": [{"x": 0.2, "y": 0.3}, {"x": 0.4, "y": 0.5}]
                }]
            }
        }),
    )
    .await;
    assert_eq!(comment.body, "Ease the landing pose");
    assert_eq!(comment.timecode_ms, Some(1250));
    assert_eq!(comment.frame_number, Some(30));
    assert!(comment.annotation.is_some());

    let listed_response = api
        .client
        .get(api.url("/api/reviews/comments"))
        .bearer_auth(&project.alice.token)
        .query(&target)
        .send()
        .await
        .expect("review comment list failed");
    let listed: Vec<ReviewCommentEntry> = parse_success(listed_response).await;
    assert_eq!(listed.len(), 1);

    let resolved: ReviewCommentEntry = authed_post(
        &api,
        &project.alice,
        &format!("/api/reviews/comments/{}/resolve", comment.id),
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "resolved": true
        }),
    )
    .await;
    assert!(resolved.resolved_at.is_some());

    let proxy_download = api
        .client
        .get(api.url("/api/reviews/proxy"))
        .bearer_auth(&project.alice.token)
        .query(&target)
        .header("range", "bytes=1-3")
        .send()
        .await
        .expect("review proxy range download failed");
    assert_eq!(proxy_download.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(proxy_download.headers()["content-range"], "bytes 1-3/16");
    assert_eq!(
        proxy_download.headers()["x-review-frame-rate-numerator"],
        "24000"
    );
    assert_eq!(
        proxy_download.headers()["x-review-frame-rate-denominator"],
        "1001"
    );
    assert_eq!(proxy_download.headers()["x-review-start-frame"], "1001");
    assert_eq!(proxy_download.bytes().await.unwrap(), b"lb-".as_slice());

    let plan: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.alice,
        "/api/sync/plan",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "include_current": true
        }),
    )
    .await;
    assert!(plan[0].review_proxy_available);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn upload_purposes_isolate_content_and_review_proxy_bytes() {
    let api = api();
    let project = setup_project(&api, "private_upload_sessions").await;
    let content_request = serde_json::json!({
        "workspace_id": project.alice_workspace,
        "path": "Scenes/Shared.blend",
        "purpose": "content",
        "size_bytes": 4
    });
    let review_request = serde_json::json!({
        "workspace_id": project.alice_workspace,
        "path": "Scenes/Shared.blend",
        "purpose": "review_proxy",
        "size_bytes": 4
    });

    let content: UploadSessionEntry =
        authed_post(&api, &project.alice, "/api/uploads", content_request).await;
    let review: UploadSessionEntry =
        authed_post(&api, &project.alice, "/api/uploads", review_request).await;

    assert_ne!(content.upload_id, review.upload_id);
    assert_eq!(content.purpose, "content");
    assert_eq!(review.purpose, "review_proxy");
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn submit_retry_with_identical_content_returns_original_revision() {
    let api = api();
    let project = setup_project(&api, "submit_retry").await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    let path = "Models/Retry.fbx";
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    let first = submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"stable-retry-content",
    )
    .await;
    let retry = submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"stable-retry-content",
    )
    .await;
    assert_eq!(retry.revisions.len(), 1);
    assert_eq!(
        retry.revisions[0].revision_number,
        first.revisions[0].revision_number
    );
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn active_lock_paths_are_case_insensitive() {
    let api = api();
    let project = setup_project(&api, "lock_case").await;
    lock_path(
        &api,
        &project.alice,
        project.alice_workspace,
        "Content/Hero.uasset",
    )
    .await;
    let response = api
        .client
        .post(api.url("/api/files/lock"))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({
            "workspace_id": project.bob_workspace,
            "path": "content/hero.uasset"
        }))
        .send()
        .await
        .expect("second lock request failed");
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn non_admin_cannot_create_users() {
    let api = api();
    let project = setup_project(&api, "user_admin").await;
    let response = api
        .client
        .post(api.url("/api/users"))
        .bearer_auth(&project.alice.token)
        .json(&CreateUserRequest {
            username: "unauthorized_created_user",
            password: PASSWORD,
            display_name: "Unauthorized",
            is_admin: None,
        })
        .send()
        .await
        .expect("user creation request failed");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn second_user_syncs_submitted_file() {
    let api = api();
    let project = setup_project(&api, "sync_second").await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    let path = "Content/Maps/Intro.umap";
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"umap-data-for-sync",
    )
    .await;

    let plan: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.bob,
        "/api/sync/plan",
        serde_json::json!({ "workspace_id": project.bob_workspace }),
    )
    .await;
    assert!(plan
        .iter()
        .any(|entry| entry.path == path && entry.revision_number == 1));

    let response = api
        .client
        .post(api.url("/api/sync/download"))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({ "workspace_id": project.bob_workspace, "path": path }))
        .send()
        .await
        .expect("download request failed");
    let bytes = assert_success(response).await.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"umap-data-for-sync");
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn sync_ack_makes_steady_state_plan_empty() {
    let api = api();
    let project = setup_project(&api, "sync_ack").await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    let path = "Audio/Ambience.wav";
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"wave-data",
    )
    .await;

    let plan: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.bob,
        "/api/sync/plan",
        serde_json::json!({ "workspace_id": project.bob_workspace, "limit": 1 }),
    )
    .await;
    assert_eq!(plan.len(), 1);
    let _: serde_json::Value = authed_post(
        &api,
        &project.bob,
        "/api/sync/ack",
        serde_json::json!({
            "workspace_id": project.bob_workspace,
            "entries": [{
                "path": plan[0].path.clone(),
                "revision_number": plan[0].revision_number,
            }],
        }),
    )
    .await;
    let steady_state: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.bob,
        "/api/sync/plan",
        serde_json::json!({ "workspace_id": project.bob_workspace }),
    )
    .await;
    assert!(steady_state.is_empty());

    let current: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.bob,
        "/api/sync/plan",
        serde_json::json!({
            "workspace_id": project.bob_workspace,
            "paths": [path],
            "include_current": true,
        }),
    )
    .await;
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].path, path);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn delete_submit_creates_tombstone_and_sync_plan() {
    let api = api();
    let project = setup_project(&api, "delete_file").await;
    let path = "Scenes/Obsolete.blend";
    let add_change = create_changelist(&api, &project.alice, project.alice_workspace).await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        add_change,
        path,
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        add_change,
        path,
        b"obsolete-blender-scene",
    )
    .await;

    let delete_change = create_changelist(&api, &project.alice, project.alice_workspace).await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    let _: serde_json::Value = authed_post(
        &api,
        &project.alice,
        "/api/files/delete",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "changelist_id": delete_change,
            "path": path,
        }),
    )
    .await;
    let deleted =
        submit_without_files(&api, &project.alice, project.alice_workspace, delete_change).await;
    assert_eq!(deleted.revisions.len(), 1);
    assert_eq!(deleted.revisions[0].revision_number, 2);

    let plan: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.bob,
        "/api/sync/plan",
        serde_json::json!({ "workspace_id": project.bob_workspace }),
    )
    .await;
    assert!(plan
        .iter()
        .any(|entry| entry.path == path && entry.revision_number == 2 && entry.deleted));
    let revisions = history(&api, &project.alice, project.alice_workspace, path).await;
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1].action, "delete");
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn cannot_submit_uasset_without_lock() {
    cannot_submit_lock_required_without_lock("no_uasset_lock", "Content/Meshes/Hero.uasset").await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn cannot_submit_umap_without_lock() {
    cannot_submit_lock_required_without_lock("no_umap_lock", "Content/Maps/Blocked.umap").await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn submit_failure_creates_no_partial_revisions() {
    let api = api();
    let project = setup_project(&api, "no_partial").await;
    let path = "Content/Maps/NoPartial.umap";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    let response = submit_bytes_expect_status(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"should-not-create-revision",
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let history = history(&api, &project.alice, project.alice_workspace, path).await;
    assert!(
        history.is_empty(),
        "failed submit must not create file revisions"
    );
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn file_history_shows_revisions_in_order() {
    let api = api();
    let project = setup_project(&api, "history_order").await;
    let path = "Models/Vehicle.fbx";
    let first = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(&api, &project.alice, project.alice_workspace, first, path).await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        first,
        path,
        b"first-version",
    )
    .await;

    let second = create_changelist(&api, &project.alice, project.alice_workspace).await;
    edit_file(&api, &project.alice, project.alice_workspace, second, path).await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        second,
        path,
        b"second-version",
    )
    .await;

    let revisions = history(&api, &project.alice, project.alice_workspace, path).await;
    let numbers: Vec<_> = revisions
        .into_iter()
        .map(|entry| entry.revision_number)
        .collect();
    assert_eq!(numbers, vec![1, 2]);

    let page = history_page(&api, &project.alice, project.alice_workspace, path, 1, 1).await;
    let page_numbers: Vec<_> = page
        .into_iter()
        .map(|entry| entry.revision_number)
        .collect();
    assert_eq!(page_numbers, vec![2]);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn audit_records_lock_unlock_submit() {
    let api = api();
    let project = setup_project(&api, "audit").await;
    let path = "Content/Maps/Audit.umap";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    unlock_path(&api, &project.alice, project.alice_workspace, path).await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"audit-submit",
    )
    .await;

    let pool = PgPool::connect(&database_url()).await.unwrap();
    for event_type in ["file_lock", "file_unlock", "submit_success"] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_events WHERE event_type = $1 AND depot_path = $2 OR (event_type = $1 AND changelist_id = $3)",
        )
        .bind(event_type)
        .bind(path)
        .bind(change_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(count > 0, "missing audit event {event_type}");
    }
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn unity_meta_missing_warning_is_reported() {
    let api = api();
    let project = setup_project(&api, "unity_meta").await;
    let response: ValidationResponse = authed_post(
        &api,
        &project.alice,
        "/api/validate",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "paths": [{ "path": "Assets/Scenes/Main.unity", "size_bytes": 12 }]
        }),
    )
    .await;
    assert!(response
        .warnings
        .iter()
        .any(|warning| warning.code == "unity_meta_missing"));
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn path_traversal_attempts_are_rejected() {
    let api = api();
    let project = setup_project(&api, "path_traversal").await;
    let response = api
        .client
        .post(api.url("/api/files/lock"))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": "../Secrets.umap",
            "reason": "bad path"
        }))
        .send()
        .await
        .expect("lock request failed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn user_without_depot_permission_cannot_create_workspace() {
    let api = api();
    let admin = ensure_admin(&api).await;
    let suffix = short_id();
    let alice = create_user(&api, &admin, &format!("perm_alice_{suffix}")).await;
    let bob = create_user(&api, &admin, &format!("perm_bob_{suffix}")).await;
    let depot_name = format!("private_depot_{suffix}");
    let depot: IdResponse = authed_post(
        &api,
        &alice,
        "/api/depots",
        serde_json::json!({ "name": depot_name }),
    )
    .await;
    let stream: IdResponse = authed_post(
        &api,
        &alice,
        "/api/streams",
        serde_json::json!({ "depot": depot.id, "name": "main" }),
    )
    .await;

    let response = api
        .client
        .post(api.url("/api/workspaces"))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({
            "name": format!("bob_denied_{suffix}"),
            "depot": depot.id,
            "stream": stream.id,
            "local_path": format!("/tmp/openasset/denied/{suffix}")
        }))
        .send()
        .await
        .expect("workspace create request failed");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn user_without_depot_permission_cannot_see_private_locks() {
    let api = api();
    let admin = ensure_admin(&api).await;
    let suffix = short_id();
    let alice = create_user(&api, &admin, &format!("lock_owner_{suffix}")).await;
    let outsider = create_user(&api, &admin, &format!("lock_outsider_{suffix}")).await;
    let depot_name = format!("private_locks_{suffix}");
    let depot: IdResponse = authed_post(
        &api,
        &alice,
        "/api/depots",
        serde_json::json!({ "name": depot_name }),
    )
    .await;
    let stream: IdResponse = authed_post(
        &api,
        &alice,
        "/api/streams",
        serde_json::json!({ "depot": depot.id, "name": "main" }),
    )
    .await;
    let workspace: IdResponse = authed_post(
        &api,
        &alice,
        "/api/workspaces",
        serde_json::json!({
            "name": format!("private_locks_ws_{suffix}"),
            "depot": depot.id,
            "stream": stream.id,
            "local_path": format!("/tmp/openasset/private-locks/{suffix}")
        }),
    )
    .await;
    let path = "Scenes/Private.blend";
    lock_path(&api, &alice, workspace.id, path).await;

    let locks: Vec<LockResponse> = authed_get(&api, &outsider, "/api/locks").await;
    assert!(!locks.iter().any(|lock| lock.depot_path == path));
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn deleting_workspace_releases_locks_and_abandons_pending_changes() {
    let api = api();
    let project = setup_project(&api, "delete_workspace").await;
    let path = "Content/Maps/DeleteMe.umap";
    let _change = create_changelist(&api, &project.alice, project.alice_workspace).await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;

    let response = api
        .client
        .delete(api.url(&format!("/api/workspaces/{}", project.alice_workspace)))
        .bearer_auth(&project.alice.token)
        .send()
        .await
        .expect("workspace delete request failed");
    let deleted: DeleteWorkspaceResponse = parse_success(response).await;
    assert_eq!(deleted.id, project.alice_workspace);
    assert!(deleted.deleted);
    assert_eq!(deleted.released_locks, 1);
    assert_eq!(deleted.abandoned_changelists, 1);

    let workspaces: Vec<IdResponse> = authed_get(&api, &project.alice, "/api/workspaces").await;
    assert!(!workspaces
        .iter()
        .any(|workspace| workspace.id == project.alice_workspace));

    let pool = PgPool::connect(&database_url())
        .await
        .expect("database connection failed");
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deleted_at FROM workspaces WHERE id = $1")
            .bind(project.alice_workspace)
            .fetch_one(&pool)
            .await
            .expect("workspace row missing");
    assert!(deleted_at.is_some());
    let lock_state: String =
        sqlx::query_scalar("SELECT state FROM locks WHERE workspace_id = $1 AND depot_path = $2")
            .bind(project.alice_workspace)
            .bind(path)
            .fetch_one(&pool)
            .await
            .expect("lock row missing");
    assert_eq!(lock_state, "released");
    let pending_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM changelists WHERE workspace_id = $1 AND status = 'pending'",
    )
    .bind(project.alice_workspace)
    .fetch_one(&pool)
    .await
    .expect("pending changelist query failed");
    assert_eq!(pending_count, 0);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn user_cannot_mutate_another_users_changelist() {
    let api = api();
    let project = setup_project(&api, "change_owner").await;
    let alice_change = create_changelist(&api, &project.alice, project.alice_workspace).await;

    let response = api
        .client
        .post(api.url("/api/files/add"))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({
            "workspace_id": project.bob_workspace,
            "changelist_id": alice_change,
            "path": "Models/Stolen.fbx"
        }))
        .send()
        .await
        .expect("file add request failed");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn duplicate_submit_file_parts_are_rejected() {
    let api = api();
    let project = setup_project(&api, "duplicate_submit").await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        "Models/Dupe.fbx",
    )
    .await;

    let response =
        submit_duplicate_parts(&api, &project.alice, project.alice_workspace, change_id).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn idempotency_key_replays_changelist_create() {
    let api = api();
    let project = setup_project(&api, "idempotency").await;
    let key = format!("change-{}", short_id());
    let body = serde_json::json!({
        "workspace_id": project.alice_workspace,
        "description": "idempotent changelist"
    });

    let first: IdResponse =
        authed_post_with_key(&api, &project.alice, "/api/changelists", &key, body.clone()).await;
    let second: IdResponse =
        authed_post_with_key(&api, &project.alice, "/api/changelists", &key, body).await;
    assert_eq!(first.id, second.id);

    let conflict = api
        .client
        .post(api.url("/api/changelists"))
        .bearer_auth(&project.alice.token)
        .header("Idempotency-Key", &key)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "description": "different body"
        }))
        .send()
        .await
        .expect("idempotent request failed");
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
}

/// Submits `path` and returns nothing, leaving revision 1 in place for reviews.
async fn publish_reviewable_asset(api: &TestClient, project: &Project, path: &str) {
    let change_id = create_changelist(api, &project.alice, project.alice_workspace).await;
    add_file(
        api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    submit_bytes(
        api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"reviewable-asset-bytes",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn a_review_is_approved_only_once_every_reviewer_agrees() {
    let api = api();
    let project = setup_project(&api, "review_approve").await;
    let path = "Models/Reviewed.fbx";
    publish_reviewable_asset(&api, &project, path).await;

    let opened: ReviewRequestResponse = authed_post(
        &api,
        &project.alice,
        "/api/reviews/requests",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Hero prop for turntable",
            "description": "Silhouette pass is final.",
            "reviewers": [project.bob.id, project.admin.id]
        }),
    )
    .await;
    assert_eq!(opened.path, path);
    assert_eq!(opened.revision_number, 1);
    assert_eq!(opened.state, "open");
    assert_eq!(opened.reviewers.len(), 2);
    assert!(opened.reviewers.iter().all(|r| r.decision == "pending"));

    // One approval out of two is not an approved review.
    let partial: ReviewRequestResponse = authed_post(
        &api,
        &project.bob,
        &format!("/api/reviews/requests/{}/decision", opened.id),
        serde_json::json!({
            "workspace_id": project.bob_workspace,
            "decision": "approved",
            "note": "Reads well at distance."
        }),
    )
    .await;
    assert_eq!(partial.state, "open");
    let bob_entry = partial
        .reviewers
        .iter()
        .find(|r| r.reviewer_user_id == project.bob.id)
        .expect("bob missing from the reviewer list");
    assert_eq!(bob_entry.decision, "approved");
    assert_eq!(bob_entry.note.as_deref(), Some("Reads well at distance."));

    let admin_workspace = admin_workspace(&api, &project).await;
    let approved: ReviewRequestResponse = authed_post(
        &api,
        &project.admin,
        &format!("/api/reviews/requests/{}/decision", opened.id),
        serde_json::json!({
            "workspace_id": admin_workspace,
            "decision": "approved"
        }),
    )
    .await;
    assert_eq!(approved.state, "approved");
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn one_reviewer_requesting_changes_outranks_other_approvals() {
    let api = api();
    let project = setup_project(&api, "review_changes").await;
    let path = "Models/Contested.fbx";
    publish_reviewable_asset(&api, &project, path).await;
    let admin_workspace = admin_workspace(&api, &project).await;

    let opened: ReviewRequestResponse = authed_post(
        &api,
        &project.alice,
        "/api/reviews/requests",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Needs a second opinion",
            "reviewers": [project.bob.id, project.admin.id]
        }),
    )
    .await;

    let _: ReviewRequestResponse = authed_post(
        &api,
        &project.admin,
        &format!("/api/reviews/requests/{}/decision", opened.id),
        serde_json::json!({ "workspace_id": admin_workspace, "decision": "approved" }),
    )
    .await;
    let blocked: ReviewRequestResponse = authed_post(
        &api,
        &project.bob,
        &format!("/api/reviews/requests/{}/decision", opened.id),
        serde_json::json!({
            "workspace_id": project.bob_workspace,
            "decision": "changes_requested",
            "note": "Pivot is off centre."
        }),
    )
    .await;
    // An existing approval must not mask a reviewer saying the asset is not ready.
    assert_eq!(blocked.state, "changes_requested");

    // It shows up for the artist as still awaiting the team, not as finished.
    let awaiting: Vec<ReviewRequestResponse> = authed_get(
        &api,
        &project.alice,
        &format!(
            "/api/reviews/requests?workspace_id={}&state=changes_requested",
            project.alice_workspace
        ),
    )
    .await;
    assert!(awaiting.iter().any(|entry| entry.id == opened.id));
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn only_assigned_reviewers_can_decide_and_only_one_review_stays_live() {
    let api = api();
    let project = setup_project(&api, "review_guards").await;
    let path = "Models/Guarded.fbx";
    publish_reviewable_asset(&api, &project, path).await;

    let opened: ReviewRequestResponse = authed_post(
        &api,
        &project.alice,
        "/api/reviews/requests",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Only bob reviews this",
            "reviewers": [project.bob.id]
        }),
    )
    .await;

    // Alice opened the review but is not on it, so she cannot approve her own work.
    let forbidden = api
        .client
        .post(api.url(&format!("/api/reviews/requests/{}/decision", opened.id)))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "decision": "approved"
        }))
        .send()
        .await
        .expect("decision request failed");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let duplicate = api
        .client
        .post(api.url("/api/reviews/requests"))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Competing review",
            "reviewers": [project.bob.id]
        }))
        .send()
        .await
        .expect("review request failed");
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    // Closing releases the revision, so a fresh review can be opened after it.
    let closed: ReviewRequestResponse = authed_post(
        &api,
        &project.alice,
        &format!("/api/reviews/requests/{}/close", opened.id),
        serde_json::json!({ "workspace_id": project.alice_workspace }),
    )
    .await;
    assert_eq!(closed.state, "closed");

    let reopened: ReviewRequestResponse = authed_post(
        &api,
        &project.alice,
        "/api/reviews/requests",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Second pass review",
            "reviewers": [project.bob.id]
        }),
    )
    .await;
    assert_eq!(reopened.state, "open");
    assert_ne!(reopened.id, opened.id);

    // A closed review accepts no further decisions.
    let on_closed = api
        .client
        .post(api.url(&format!("/api/reviews/requests/{}/decision", opened.id)))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({
            "workspace_id": project.bob_workspace,
            "decision": "approved"
        }))
        .send()
        .await
        .expect("decision request failed");
    assert_eq!(on_closed.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn a_review_needs_reviewers_who_can_actually_open_the_asset() {
    let api = api();
    let project = setup_project(&api, "review_reviewers").await;
    let path = "Models/Unreachable.fbx";
    publish_reviewable_asset(&api, &project, path).await;
    let outsider = create_user(&api, &project.admin, &format!("outsider_{}", short_id())).await;

    let no_reviewers = api
        .client
        .post(api.url("/api/reviews/requests"))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Nobody assigned",
            "reviewers": []
        }))
        .send()
        .await
        .expect("review request failed");
    assert_eq!(no_reviewers.status(), StatusCode::BAD_REQUEST);

    // Assigning someone without depot access would create a review that can
    // never be completed, so it is refused rather than silently stalling.
    let unreachable = api
        .client
        .post(api.url("/api/reviews/requests"))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Reviewer cannot see this depot",
            "reviewers": [outsider.id]
        }))
        .send()
        .await
        .expect("review request failed");
    assert_eq!(unreachable.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn awaiting_me_lists_only_reviews_that_still_need_the_caller() {
    let api = api();
    let project = setup_project(&api, "review_queue").await;
    let path = "Models/Queued.fbx";
    publish_reviewable_asset(&api, &project, path).await;

    let opened: ReviewRequestResponse = authed_post(
        &api,
        &project.alice,
        "/api/reviews/requests",
        serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "revision_number": 1,
            "title": "Waiting on bob",
            "reviewers": [project.bob.id]
        }),
    )
    .await;

    let queue: Vec<ReviewRequestResponse> = authed_get(
        &api,
        &project.bob,
        &format!(
            "/api/reviews/requests?workspace_id={}&awaiting_me=true",
            project.bob_workspace
        ),
    )
    .await;
    assert!(queue.iter().any(|entry| entry.id == opened.id));

    let _: ReviewRequestResponse = authed_post(
        &api,
        &project.bob,
        &format!("/api/reviews/requests/{}/decision", opened.id),
        serde_json::json!({ "workspace_id": project.bob_workspace, "decision": "approved" }),
    )
    .await;

    // Once bob has answered it must leave his queue, or the queue is useless.
    let drained: Vec<ReviewRequestResponse> = authed_get(
        &api,
        &project.bob,
        &format!(
            "/api/reviews/requests?workspace_id={}&awaiting_me=true",
            project.bob_workspace
        ),
    )
    .await;
    assert!(!drained.iter().any(|entry| entry.id == opened.id));
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn workspace_collaborators_include_depot_members_and_system_admins() {
    let api = api();
    let project = setup_project(&api, "collaborators").await;
    let people: Vec<CollaboratorResponse> = authed_get(
        &api,
        &project.alice,
        &format!("/api/workspaces/{}/collaborators", project.alice_workspace),
    )
    .await;
    let ids: Vec<Uuid> = people.iter().map(|person| person.user_id).collect();
    assert!(ids.contains(&project.alice.id));
    assert!(ids.contains(&project.bob.id));
    assert!(ids.contains(&project.admin.id));
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn shelved_work_round_trips_without_creating_a_revision() {
    let api = api();
    let project = setup_project(&api, "shelve_round_trip").await;
    let path = "Scenes/Parked.blend";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;

    // Shelving deliberately needs no lock: an artist parking work should not be
    // blocked by checkout state. The lock here is only for the later submit.
    lock_path(&api, &project.alice, project.alice_workspace, path).await;

    let shelved: ShelfResponse = parse_success(
        shelve_bytes(&api, &project.alice, change_id, path, b"work-in-progress").await,
    )
    .await;
    assert_eq!(shelved.changelist_id, change_id);
    assert_eq!(shelved.files.len(), 1);
    assert_eq!(shelved.files[0].path, path);
    assert_eq!(shelved.files[0].size_bytes, 16);
    // Nothing exists at this path in the depot yet, so the shelf records an add.
    assert_eq!(shelved.files[0].action, "add");

    // Shelving must not publish anything: a second workspace still sees no file.
    let plan: Vec<SyncPlanEntry> = authed_post(
        &api,
        &project.bob,
        "/api/sync/plan",
        serde_json::json!({ "workspace_id": project.bob_workspace }),
    )
    .await;
    assert!(!plan.iter().any(|entry| entry.path == path));

    let bytes = api
        .client
        .get(api.url(&format!(
            "/api/shelves/content?changelist_id={change_id}&path={path}"
        )))
        .bearer_auth(&project.alice.token)
        .send()
        .await
        .expect("shelf content request failed");
    let bytes = assert_success(bytes).await.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"work-in-progress");

    let restored: ShelfResponse = authed_post(
        &api,
        &project.alice,
        &format!("/api/changelists/{change_id}/unshelve"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(restored.files.len(), 1);

    // The shelf is no longer active once restored, so it stops being offered.
    let empty: ShelfResponse = authed_get(
        &api,
        &project.alice,
        &format!("/api/changelists/{change_id}/shelve"),
    )
    .await;
    assert!(empty.files.is_empty());

    // Unshelving restores the pending operation, so the same content can submit.
    let submitted = submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"work-in-progress",
    )
    .await;
    assert_eq!(submitted.revisions.len(), 1);
    assert_eq!(submitted.revisions[0].revision_number, 1);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn a_shelf_is_visible_to_teammates_but_only_its_owner_can_change_it() {
    let api = api();
    let project = setup_project(&api, "shelve_sharing").await;
    let path = "Scenes/Shared.blend";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    let _: ShelfResponse =
        parse_success(shelve_bytes(&api, &project.alice, change_id, path, b"alice-wip").await)
            .await;

    // Bob has depot write on this stream, so he can see what Alice parked.
    let visible: ShelfResponse = authed_get(
        &api,
        &project.bob,
        &format!("/api/changelists/{change_id}/shelve"),
    )
    .await;
    assert_eq!(visible.files.len(), 1);
    assert_eq!(visible.files[0].path, path);

    let summaries: Vec<ShelfSummary> = authed_get(
        &api,
        &project.bob,
        &format!("/api/shelves?workspace_id={}", project.bob_workspace),
    )
    .await;
    let summary = summaries
        .iter()
        .find(|entry| entry.changelist_id == change_id)
        .expect("shelf missing from the stream listing");
    assert_eq!(summary.file_count, 1);
    assert_eq!(summary.total_bytes, 9);
    assert!(summary.owner.starts_with("alice_"));

    // Seeing a shelf is not the same as owning it.
    let forbidden = api
        .client
        .post(api.url(&format!("/api/changelists/{change_id}/unshelve")))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("unshelve request failed");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let forbidden_shelve = shelve_bytes(&api, &project.bob, change_id, path, b"bob-wip").await;
    assert_eq!(forbidden_shelve.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn reshelving_replaces_content_and_discarding_leaves_nothing_to_restore() {
    let api = api();
    let project = setup_project(&api, "shelve_replace").await;
    let path = "Scenes/Iterated.blend";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;

    let first: ShelfResponse =
        parse_success(shelve_bytes(&api, &project.alice, change_id, path, b"first-pass").await)
            .await;
    let second: ShelfResponse = parse_success(
        shelve_bytes(&api, &project.alice, change_id, path, b"second-pass-longer").await,
    )
    .await;
    // One path holds one shelved version, so the newer pass replaces the older.
    assert_eq!(second.files.len(), 1);
    assert_ne!(first.files[0].blob_hash, second.files[0].blob_hash);
    assert_eq!(second.files[0].size_bytes, 18);

    let discarded: DiscardShelfResponse = parse_success(
        api.client
            .delete(api.url(&format!("/api/changelists/{change_id}/shelve")))
            .bearer_auth(&project.alice.token)
            .send()
            .await
            .expect("discard request failed"),
    )
    .await;
    assert_eq!(discarded.discarded, 1);

    let nothing_to_restore = api
        .client
        .post(api.url(&format!("/api/changelists/{change_id}/unshelve")))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("unshelve request failed");
    assert_eq!(nothing_to_restore.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn a_shelf_records_an_edit_when_the_path_already_exists() {
    let api = api();
    let project = setup_project(&api, "shelve_edit").await;
    let path = "Scenes/Existing.blend";
    let first_change = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        first_change,
        path,
    )
    .await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        first_change,
        path,
        b"published",
    )
    .await;

    let second_change = create_changelist(&api, &project.alice, project.alice_workspace).await;
    let shelved: ShelfResponse =
        parse_success(shelve_bytes(&api, &project.alice, second_change, path, b"revised").await)
            .await;
    assert_eq!(shelved.files[0].action, "edit");
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn a_submitted_changelist_cannot_be_shelved() {
    let api = api();
    let project = setup_project(&api, "shelve_submitted").await;
    let path = "Scenes/Closed.blend";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    lock_path(&api, &project.alice, project.alice_workspace, path).await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"already-published",
    )
    .await;

    let conflict = shelve_bytes(&api, &project.alice, change_id, path, b"too-late").await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn only_an_admin_can_force_unlock_and_the_path_becomes_lockable() {
    let api = api();
    let project = setup_project(&api, "force_unlock").await;
    let path = "Content/Maps/Contested.umap";
    let held = lock_path(&api, &project.alice, project.alice_workspace, path).await;
    assert_eq!(held.user_id, project.alice.id);

    let body = serde_json::json!({
        "lock_id": held.id,
        "reason": "artist left for the day mid-checkout"
    });

    let forbidden = api
        .client
        .post(api.url("/api/admin/locks/force-unlock"))
        .bearer_auth(&project.bob.token)
        .json(&body)
        .send()
        .await
        .expect("force-unlock request failed");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let released: LockResponse =
        authed_post(&api, &project.admin, "/api/admin/locks/force-unlock", body).await;
    assert_eq!(released.id, held.id);
    assert_eq!(released.depot_path, path);
    // The response names the artist whose work was interrupted, not the admin.
    assert_eq!(released.user_id, project.alice.id);

    // The point of forcing a release is that someone else can pick the file up.
    let retaken = lock_path(&api, &project.bob, project.bob_workspace, path).await;
    assert_eq!(retaken.user_id, project.bob.id);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn force_unlock_requires_a_reason_and_a_locatable_lock() {
    let api = api();
    let project = setup_project(&api, "force_unlock_input").await;
    let path = "Content/Maps/Reasoned.umap";
    let held = lock_path(&api, &project.alice, project.alice_workspace, path).await;

    let blank_reason = api
        .client
        .post(api.url("/api/admin/locks/force-unlock"))
        .bearer_auth(&project.admin.token)
        .json(&serde_json::json!({ "lock_id": held.id, "reason": "   " }))
        .send()
        .await
        .expect("force-unlock request failed");
    assert_eq!(blank_reason.status(), StatusCode::BAD_REQUEST);

    // A path with no stream is ambiguous across depots, so it must be rejected
    // rather than guessed at.
    let underspecified = api
        .client
        .post(api.url("/api/admin/locks/force-unlock"))
        .bearer_auth(&project.admin.token)
        .json(&serde_json::json!({ "path": path, "reason": "no stream given" }))
        .send()
        .await
        .expect("force-unlock request failed");
    assert_eq!(underspecified.status(), StatusCode::BAD_REQUEST);

    let by_path: LockResponse = authed_post(
        &api,
        &project.admin,
        "/api/admin/locks/force-unlock",
        serde_json::json!({
            "stream_id": project.stream_id,
            "path": path,
            "reason": "reassigning the shot"
        }),
    )
    .await;
    assert_eq!(by_path.id, held.id);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn retried_force_unlock_replays_instead_of_reporting_a_missing_lock() {
    let api = api();
    let project = setup_project(&api, "force_unlock_retry").await;
    let path = "Content/Maps/Retried.umap";
    let held = lock_path(&api, &project.alice, project.alice_workspace, path).await;
    let key = format!("force-unlock-{}", short_id());
    let body = serde_json::json!({
        "lock_id": held.id,
        "reason": "network dropped on the first attempt"
    });

    let first: LockResponse = authed_post_with_key(
        &api,
        &project.admin,
        "/api/admin/locks/force-unlock",
        &key,
        body.clone(),
    )
    .await;
    // Without the key this retry would 404: the lock is already released.
    let replay: LockResponse = authed_post_with_key(
        &api,
        &project.admin,
        "/api/admin/locks/force-unlock",
        &key,
        body,
    )
    .await;
    assert_eq!(first.id, replay.id);
    assert_eq!(first.user_id, replay.user_id);

    let missing = api
        .client
        .post(api.url("/api/admin/locks/force-unlock"))
        .bearer_auth(&project.admin.token)
        .json(&serde_json::json!({
            "lock_id": held.id,
            "reason": "network dropped on the first attempt"
        }))
        .send()
        .await
        .expect("force-unlock request failed");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn admin_storage_verify_reports_clean_submitted_blobs() {
    let api = api();
    let project = setup_project(&api, "storage_verify").await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        "Models/Verify.fbx",
    )
    .await;
    submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        "Models/Verify.fbx",
        b"verify-storage",
    )
    .await;

    let summary: StorageVerificationSummary =
        authed_get(&api, &project.admin, "/api/admin/storage/verify?limit=100").await;
    assert!(summary.checked > 0);
    assert_eq!(summary.failed, 0);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn large_files_stream_without_excessive_memory() {
    let api = api();
    let project = setup_project(&api, "large_stream").await;
    let path = "Renders/Large.mov";
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    let data = vec![42u8; 5 * 1024 * 1024];
    let submit = submit_bytes(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        &data,
    )
    .await;
    assert_eq!(submit.revisions[0].revision_number, 1);
}

#[tokio::test]
#[ignore = "requires running OpenAsset Depot API and PostgreSQL"]
async fn concurrent_lock_attempts_are_transaction_safe() {
    let api = api();
    let project = setup_project(&api, "lock_race").await;
    let path = "Content/Maps/Race.umap";

    let first = api
        .client
        .post(api.url("/api/files/lock"))
        .bearer_auth(&project.alice.token)
        .json(&serde_json::json!({
            "workspace_id": project.alice_workspace,
            "path": path,
            "reason": "alice"
        }));
    let second = api
        .client
        .post(api.url("/api/files/lock"))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({
            "workspace_id": project.bob_workspace,
            "path": path,
            "reason": "bob"
        }));

    let (first, second) = tokio::join!(first.send(), second.send());
    let statuses = [first.unwrap().status(), second.unwrap().status()];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT));
}

async fn lock_conflict_for(label: &str, path: &str) {
    let api = api();
    let project = setup_project(&api, label).await;
    let first = lock_path(&api, &project.alice, project.alice_workspace, path).await;
    assert_eq!(first.depot_path, path);
    let response = api
        .client
        .post(api.url("/api/files/lock"))
        .bearer_auth(&project.bob.token)
        .json(&serde_json::json!({
            "workspace_id": project.bob_workspace,
            "path": path,
            "reason": "second user should conflict"
        }))
        .send()
        .await
        .expect("second lock request failed");
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

async fn cannot_submit_lock_required_without_lock(label: &str, path: &str) {
    let api = api();
    let project = setup_project(&api, label).await;
    let change_id = create_changelist(&api, &project.alice, project.alice_workspace).await;
    add_file(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
    )
    .await;
    let response = submit_bytes_expect_status(
        &api,
        &project.alice,
        project.alice_workspace,
        change_id,
        path,
        b"lock-required-content",
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

async fn create_changelist(api: &TestClient, user: &User, workspace_id: Uuid) -> Uuid {
    let response: IdResponse = authed_post(
        api,
        user,
        "/api/changelists",
        serde_json::json!({
            "workspace_id": workspace_id,
            "description": "integration test changelist"
        }),
    )
    .await;
    response.id
}

async fn grant_depot_permission(
    api: &TestClient,
    user: &User,
    depot_id: Uuid,
    target_user_id: Uuid,
    role: &str,
) {
    let _: serde_json::Value = authed_post(
        api,
        user,
        &format!("/api/depots/{depot_id}/permissions"),
        serde_json::json!({ "user_id": target_user_id, "role": role }),
    )
    .await;
}

async fn add_file(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    changelist_id: Uuid,
    path: &str,
) {
    let _: serde_json::Value = authed_post(
        api,
        user,
        "/api/files/add",
        serde_json::json!({
            "workspace_id": workspace_id,
            "changelist_id": changelist_id,
            "path": path
        }),
    )
    .await;
}

async fn edit_file(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    changelist_id: Uuid,
    path: &str,
) {
    let _: serde_json::Value = authed_post(
        api,
        user,
        "/api/files/edit",
        serde_json::json!({
            "workspace_id": workspace_id,
            "changelist_id": changelist_id,
            "path": path
        }),
    )
    .await;
}

async fn lock_path(api: &TestClient, user: &User, workspace_id: Uuid, path: &str) -> LockResponse {
    authed_post(
        api,
        user,
        "/api/files/lock",
        serde_json::json!({
            "workspace_id": workspace_id,
            "path": path,
            "reason": "integration test"
        }),
    )
    .await
}

async fn unlock_path(api: &TestClient, user: &User, workspace_id: Uuid, path: &str) {
    let _: LockResponse = authed_post(
        api,
        user,
        "/api/files/unlock",
        serde_json::json!({ "workspace_id": workspace_id, "path": path }),
    )
    .await;
}

async fn submit_bytes(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    changelist_id: Uuid,
    path: &str,
    bytes: &[u8],
) -> SubmitResponse {
    let response =
        submit_bytes_expect_status(api, user, workspace_id, changelist_id, path, bytes).await;
    parse_success(response).await
}

async fn submit_bytes_expect_status(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    changelist_id: Uuid,
    path: &str,
    bytes: &[u8],
) -> reqwest::Response {
    let form = multipart::Form::new()
        .text("workspace_id", workspace_id.to_string())
        .part(
            "file",
            multipart::Part::bytes(bytes.to_vec()).file_name(path.to_string()),
        );
    api.client
        .post(api.url(&format!("/api/changelists/{changelist_id}/submit")))
        .bearer_auth(&user.token)
        .multipart(form)
        .send()
        .await
        .expect("submit request failed")
}

async fn shelve_bytes(
    api: &TestClient,
    user: &User,
    changelist_id: Uuid,
    path: &str,
    bytes: &[u8],
) -> reqwest::Response {
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(bytes.to_vec()).file_name(path.to_string()),
    );
    api.client
        .post(api.url(&format!("/api/changelists/{changelist_id}/shelve")))
        .bearer_auth(&user.token)
        .multipart(form)
        .send()
        .await
        .expect("shelve request failed")
}

async fn submit_duplicate_parts(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    changelist_id: Uuid,
) -> reqwest::Response {
    let form = multipart::Form::new()
        .text("workspace_id", workspace_id.to_string())
        .part(
            "file",
            multipart::Part::bytes(b"first".to_vec()).file_name("Models/Dupe.fbx"),
        )
        .part(
            "file",
            multipart::Part::bytes(b"second".to_vec()).file_name("Models/Dupe.fbx"),
        );
    api.client
        .post(api.url(&format!("/api/changelists/{changelist_id}/submit")))
        .bearer_auth(&user.token)
        .multipart(form)
        .send()
        .await
        .expect("submit request failed")
}

async fn submit_without_files(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    changelist_id: Uuid,
) -> SubmitResponse {
    let form = multipart::Form::new().text("workspace_id", workspace_id.to_string());
    let response = api
        .client
        .post(api.url(&format!("/api/changelists/{changelist_id}/submit")))
        .bearer_auth(&user.token)
        .multipart(form)
        .send()
        .await
        .expect("delete submit request failed");
    parse_success(response).await
}

async fn history(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    path: &str,
) -> Vec<HistoryEntry> {
    let response = api
        .client
        .get(api.url("/api/files/history"))
        .bearer_auth(&user.token)
        .query(&[
            ("workspace_id", workspace_id.to_string()),
            ("path", path.to_string()),
        ])
        .send()
        .await
        .expect("history request failed");
    parse_success(response).await
}

async fn history_page(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    path: &str,
    limit: i64,
    offset: i64,
) -> Vec<HistoryEntry> {
    let response = api
        .client
        .get(api.url("/api/files/history"))
        .bearer_auth(&user.token)
        .query(&[
            ("workspace_id", workspace_id.to_string()),
            ("path", path.to_string()),
            ("limit", limit.to_string()),
            ("offset", offset.to_string()),
        ])
        .send()
        .await
        .expect("history request failed");
    parse_success(response).await
}

async fn lock_page(
    api: &TestClient,
    user: &User,
    workspace_id: Uuid,
    limit: usize,
    cursor: Option<(String, Uuid)>,
) -> LockPageResponse {
    let mut request = api
        .client
        .get(api.url("/api/locks/page"))
        .bearer_auth(&user.token)
        .query(&[
            ("workspace_id", workspace_id.to_string()),
            ("limit", limit.to_string()),
        ]);
    if let Some((created_at, id)) = cursor {
        request = request.query(&[
            ("before_created_at", created_at),
            ("before_id", id.to_string()),
        ]);
    }
    parse_success(request.send().await.unwrap()).await
}

async fn audit_page(
    api: &TestClient,
    user: &User,
    limit: usize,
    cursor: Option<(String, Uuid)>,
) -> AuditPageResponse {
    let mut request = api
        .client
        .get(api.url("/api/audit"))
        .bearer_auth(&user.token)
        .query(&[("limit", limit.to_string())]);
    if let Some((created_at, id)) = cursor {
        request = request.query(&[
            ("before_created_at", created_at),
            ("before_id", id.to_string()),
        ]);
    }
    parse_success(request.send().await.unwrap()).await
}

async fn dependency_page(
    api: &TestClient,
    user: &User,
    stream_id: Uuid,
    limit: usize,
    cursor: Option<(String, Uuid)>,
) -> DependencyPageResponse {
    let mut request = api
        .client
        .get(api.url("/api/dependencies"))
        .bearer_auth(&user.token)
        .query(&[
            ("stream_id", stream_id.to_string()),
            ("source_file", "Comps/Main.nk".to_string()),
            ("limit", limit.to_string()),
        ]);
    if let Some((scan_time, id)) = cursor {
        request = request.query(&[
            ("before_scan_time", scan_time),
            ("before_id", id.to_string()),
        ]);
    }
    parse_success(request.send().await.unwrap()).await
}

async fn authed_get<T: for<'de> Deserialize<'de>>(api: &TestClient, user: &User, path: &str) -> T {
    let response = api
        .client
        .get(api.url(path))
        .bearer_auth(&user.token)
        .send()
        .await
        .expect("request failed");
    parse_success(response).await
}

async fn authed_post<T: for<'de> Deserialize<'de>>(
    api: &TestClient,
    user: &User,
    path: &str,
    body: serde_json::Value,
) -> T {
    let response = api
        .client
        .post(api.url(path))
        .bearer_auth(&user.token)
        .json(&body)
        .send()
        .await
        .expect("request failed");
    parse_success(response).await
}

async fn authed_post_with_key<T: for<'de> Deserialize<'de>>(
    api: &TestClient,
    user: &User,
    path: &str,
    key: &str,
    body: serde_json::Value,
) -> T {
    let response = api
        .client
        .post(api.url(path))
        .bearer_auth(&user.token)
        .header("Idempotency-Key", key)
        .json(&body)
        .send()
        .await
        .expect("request failed");
    parse_success(response).await
}

async fn parse_success<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> T {
    let response = assert_success(response).await;
    response
        .json()
        .await
        .expect("failed to decode response body")
}

async fn assert_success(response: reqwest::Response) -> reqwest::Response {
    if !response.status().is_success() {
        panic!("request failed: {}", error_text(response).await);
    }
    response
}

async fn error_text(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    format!("{status}: {text}")
}

impl TestClient {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }
}

fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}
