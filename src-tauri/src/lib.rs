mod auth;
mod minecraft;
mod models;
mod net;
mod storage;

pub fn run() {
    tauri::Builder::default()
        .manage(auth::AuthState::default())
        .manage(minecraft::MinecraftState::default())
        .invoke_handler(tauri::generate_handler![
            minecraft::list_versions,
            minecraft::list_installed,
            minecraft::is_installed,
            minecraft::install_version,
            minecraft::launch_game,
            auth::account_info,
            auth::start_login,
            auth::complete_login,
            auth::logout,
        ])
        .run(tauri::generate_context!())
        .expect("Launcher startup failed");
}
