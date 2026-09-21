mod agent;
mod auth;
mod croc;
mod github;
mod model;
mod service;
mod storage;

use service::Runtime;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[tauri::command]
fn snapshot(r: tauri::State<'_, Arc<Runtime>>) -> service::Snapshot {
    r.snapshot()
}
#[tauri::command]
async fn connect_token(r: tauri::State<'_, Arc<Runtime>>, token: String) -> Result<(), String> {
    r.inner().connect(token).await
}
#[tauri::command]
async fn connect_cli(r: tauri::State<'_, Arc<Runtime>>) -> Result<(), String> {
    r.inner().connect_cli().await
}
#[tauri::command]
async fn disconnect(r: tauri::State<'_, Arc<Runtime>>) -> Result<(), String> {
    r.logout().await
}
#[tauri::command]
async fn sync(r: tauri::State<'_, Arc<Runtime>>) -> Result<(), String> {
    r.inner().sync(true).await
}
#[tauri::command]
async fn create_chat(
    r: tauri::State<'_, Arc<Runtime>>,
    name: String,
    members: Vec<String>,
) -> Result<model::Chat, String> {
    r.inner().create_chat(name, members).await
}
#[tauri::command]
async fn import_chat(
    r: tauri::State<'_, Arc<Runtime>>,
    repo: String,
) -> Result<model::Chat, String> {
    r.inner().import_chat(repo).await
}
#[tauri::command]
async fn accept_invitation(
    r: tauri::State<'_, Arc<Runtime>>,
    invitation: u64,
) -> Result<(), String> {
    r.inner().accept_invitation(invitation).await
}
#[tauri::command]
async fn send_files(
    r: tauri::State<'_, Arc<Runtime>>,
    repo: String,
    paths: Vec<String>,
) -> Result<String, String> {
    r.inner().send(repo, paths).await
}
#[tauri::command]
async fn receive_files(
    r: tauri::State<'_, Arc<Runtime>>,
    key: String,
    directory: String,
) -> Result<String, String> {
    r.inner().receive(key, directory).await
}
#[tauri::command]
async fn retry_transfer(
    r: tauri::State<'_, Arc<Runtime>>,
    key: String,
    recipient: String,
) -> Result<(), String> {
    r.inner().retry(key, recipient).await
}
#[tauri::command]
async fn request_retry(r: tauri::State<'_, Arc<Runtime>>, key: String) -> Result<(), String> {
    r.inner().request_retry(key).await
}
#[tauri::command]
fn cancel_transfer(r: tauri::State<'_, Arc<Runtime>>, job_id: String) -> Result<(), String> {
    r.cancel(job_id)
}
#[tauri::command]
async fn mark_seen(r: tauri::State<'_, Arc<Runtime>>, repo: String) -> Result<(), String> {
    r.inner().seen(repo).await
}
#[tauri::command]
async fn inspect_files(paths: Vec<String>) -> Result<Vec<model::SharedFile>, String> {
    storage::inspect(paths)
        .await
        .map(|files| files.into_iter().map(|f| f.file).collect())
}

struct Shutdown(AtomicBool);

fn show(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(auth::AuthState::default())
        .manage(Shutdown(AtomicBool::new(false)))
        .invoke_handler(tauri::generate_handler![
            snapshot,
            connect_token,
            connect_cli,
            disconnect,
            sync,
            create_chat,
            import_chat,
            accept_invitation,
            send_files,
            receive_files,
            retry_transfer,
            request_retry,
            cancel_transfer,
            mark_seen,
            inspect_files,
            auth::begin_github_login,
            auth::poll_github_login
        ])
        .setup(|app| {
            let runtime = Runtime::new(
                app.handle().clone(),
                app.path().app_data_dir()?,
                croc::executable(&app.path().resource_dir()?),
            )?;
            app.manage(runtime.clone());
            let open = MenuItem::with_id(app, "open", "Open Cricket", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Cricket", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Cricket · files, a little closer")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if matches!(e, TrayIconEvent::Click { .. }) {
                        show(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            let check = runtime.clone();
            tauri::async_runtime::spawn(async move {
                match croc::version(&check.binary).await {
                    Ok(version) => {
                        *check.croc_version.lock().unwrap() = Some(version);
                        check.changed();
                    }
                    Err(e) => check.problem(e),
                }
            });
            let api_runtime = runtime.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = agent::serve(api_runtime.clone()).await {
                    api_runtime.problem(format!("Local agent interface unavailable: {error}"));
                }
            });
            tauri::async_runtime::spawn(async move {
                loop {
                    let mut interval = if runtime
                        .app
                        .get_webview_window("main")
                        .and_then(|w| w.is_visible().ok())
                        .unwrap_or(false)
                    {
                        30
                    } else {
                        90
                    };
                    if runtime.github().is_ok() {
                        if let Err(e) = runtime.sync(false).await {
                            runtime.problem(e);
                            interval = 120;
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("Could not start Cricket");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = &event {
            if !app.state::<Shutdown>().0.swap(true, Ordering::SeqCst) {
                api.prevent_exit();
                let runtime = app.state::<Arc<Runtime>>().inner().clone();
                let handle = app.clone();
                let code = code.unwrap_or(0);
                tauri::async_runtime::spawn(async move {
                    runtime.shutdown().await;
                    handle.exit(code);
                });
            }
        }
        if let tauri::RunEvent::Exit = event {
            if let Some(runtime) = app.try_state::<Arc<Runtime>>() {
                runtime.cancel_all();
                let _ = std::fs::remove_file(runtime.dir.join("agent.json"));
            }
        }
    });
}
