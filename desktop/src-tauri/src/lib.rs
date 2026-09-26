mod integration;
mod sso;
mod workspaces;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            integration::run_oad_integration,
            sso::await_sso_redirect,
            workspaces::configure_cli_session,
            workspaces::clear_cli_session,
            workspaces::initialize_workspace,
            workspaces::remove_workspace_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenAsset Depot desktop app");
}
