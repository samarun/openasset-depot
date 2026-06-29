mod integration;
mod workspaces;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            integration::run_oad_integration,
            workspaces::initialize_workspace,
            workspaces::remove_workspace_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenAsset Depot desktop app");
}
