mod categories;
mod deep_link;
mod games;
mod logging;
mod protocol;

use categories::{Category, CategoryInput};
use games::{
    catalog::GameManifest,
    export::ExportGamesArchiveSummary,
    import::ImportGamesArchiveSummary,
    install::{
        ClassifyGamesSummary, DeleteGamesSummary, IncomingGameFile, IncomingGamePath,
        InstallGamesSummary, ScanGamesSummary,
    },
};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size};
use tauri_plugin_deep_link::DeepLinkExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDiagnostics {
    app_data_dir: String,
    games_dir: String,
    installed_games: usize,
    total_game_bytes: u64,
    catalog_cache_exists: bool,
    catalog_cache_valid: bool,
    categories_count: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeepLinkStatus {
    status: String,
    source: String,
    url: String,
    game_id: Option<String>,
    message: String,
}

#[derive(Default)]
struct DeepLinkState {
    latest_status: Mutex<Option<DeepLinkStatus>>,
}

const DEFAULT_WINDOW_WIDTH: u32 = 1360;
const MIN_WINDOW_WIDTH: u32 = 860;

fn fit_main_window_to_screen_height(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("main window not found while fitting to screen height");
        return;
    };

    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        log::warn!("no monitor found while fitting main window to screen height");
        return;
    };

    let work_area = *monitor.work_area();
    let target_height = work_area.size.height;
    let screen_width = work_area.size.width;
    if target_height == 0 || screen_width == 0 {
        log::warn!("invalid monitor work area while fitting main window");
        return;
    }

    let min_width = MIN_WINDOW_WIDTH.min(screen_width);
    let current_width = window
        .outer_size()
        .map(|size| size.width)
        .unwrap_or(DEFAULT_WINDOW_WIDTH)
        .clamp(min_width, screen_width);
    let centered_x = work_area.position.x + ((screen_width - current_width) / 2) as i32;

    if let Err(err) = window.set_min_size(Some(Size::Physical(PhysicalSize::new(
        min_width,
        target_height,
    )))) {
        log::warn!("failed to set main window minimum screen height: {err}");
    }

    if let Err(err) = window.set_size(Size::Physical(PhysicalSize::new(
        current_width,
        target_height,
    ))) {
        log::warn!("failed to set main window screen height: {err}");
    }

    if let Err(err) = window.set_position(Position::Physical(PhysicalPosition::new(
        centered_x,
        work_area.position.y,
    ))) {
        log::warn!("failed to align main window to monitor work area: {err}");
    }
}

fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = window.show() {
            log::warn!("failed to show main window for deep link: {err}");
        }
        if let Err(err) = window.set_focus() {
            log::warn!("failed to focus main window for deep link: {err}");
        }
    }
}

fn emit_deep_link_status(
    app: &AppHandle,
    status: &str,
    source: &str,
    url: &str,
    game_id: Option<&str>,
    message: &str,
) {
    let payload = DeepLinkStatus {
        status: status.to_string(),
        source: source.to_string(),
        url: url.to_string(),
        game_id: game_id.map(ToString::to_string),
        message: message.to_string(),
    };

    if let Some(state) = app.try_state::<DeepLinkState>() {
        match state.latest_status.lock() {
            Ok(mut latest_status) => {
                latest_status.replace(payload.clone());
            }
            Err(err) => {
                log::warn!("failed to store deep-link-status for {url}: {err}");
            }
        }
    }

    if let Err(err) = app.emit("deep-link-status", payload) {
        log::warn!("failed to emit deep-link-status for {url}: {err}");
    }
}

fn handle_deep_link(app: &AppHandle, url: &str, source: &str) -> Result<(), String> {
    logging::event("deep_link_received", &[("source", source), ("url", url)]);
    emit_deep_link_status(app, "received", source, url, None, "Đã nhận deep link.");

    let command = match deep_link::parser::parse_deep_link(url) {
        Ok(command) => command,
        Err(message) => {
            logging::event(
                "deep_link_failed",
                &[
                    ("source", source),
                    ("url", url),
                    ("reason", message.as_str()),
                ],
            );
            emit_deep_link_status(app, "failed", source, url, None, &message);
            let _ = app.emit("deep-link-error", message.clone());
            return Err(message);
        }
    };

    match command {
        deep_link::parser::DeepLinkCommand::Play { game_id } => {
            match games::catalog::find_game(app, &game_id) {
                Ok(Some(_game)) => {
                    focus_main_window(app);
                    logging::event(
                        "deep_link_selected",
                        &[("source", source), ("game_id", game_id.as_str())],
                    );
                    emit_deep_link_status(
                        app,
                        "selected",
                        source,
                        url,
                        Some(&game_id),
                        &format!("Đã chọn bài tập: {game_id}"),
                    );
                    Ok(())
                }
                Ok(None) => {
                    let message = format!("Khong tim thay tro choi co ma: {game_id}");
                    logging::event(
                        "deep_link_failed",
                        &[
                            ("source", source),
                            ("game_id", game_id.as_str()),
                            ("reason", message.as_str()),
                        ],
                    );
                    focus_main_window(app);
                    emit_deep_link_status(app, "failed", source, url, Some(&game_id), &message);
                    let _ = app.emit("deep-link-error", message.clone());
                    Err(message)
                }
                Err(message) => {
                    logging::event(
                        "deep_link_failed",
                        &[
                            ("source", source),
                            ("game_id", game_id.as_str()),
                            ("reason", message.as_str()),
                        ],
                    );
                    focus_main_window(app);
                    emit_deep_link_status(app, "failed", source, url, Some(&game_id), &message);
                    let _ = app.emit("deep-link-error", message.clone());
                    Err(message)
                }
            }
        }
    }
}

#[tauri::command]
fn last_deep_link_status(app: AppHandle) -> Option<DeepLinkStatus> {
    app.state::<DeepLinkState>()
        .latest_status
        .lock()
        .ok()
        .and_then(|latest_status| latest_status.clone())
}

#[tauri::command]
fn list_games(app: AppHandle) -> Result<Vec<GameManifest>, String> {
    games::catalog::list_games(&app)
}

#[tauri::command]
fn get_app_diagnostics(app: AppHandle) -> Result<AppDiagnostics, String> {
    let cache_status = games::catalog::catalog_cache_status(&app)?;
    let games = games::catalog::list_games(&app)?;
    let categories = categories::list_categories(&app)?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Khong xac dinh duoc thu muc du lieu app: {err}"))?;
    let games_dir = games::catalog::installed_games_dir(&app)?;
    let total_game_bytes = games.iter().map(|game| game.total_bytes).sum();

    Ok(AppDiagnostics {
        app_data_dir: app_data_dir.to_string_lossy().to_string(),
        games_dir: games_dir.to_string_lossy().to_string(),
        installed_games: games.len(),
        total_game_bytes,
        catalog_cache_exists: cache_status.exists,
        catalog_cache_valid: cache_status.valid,
        categories_count: categories.len(),
    })
}

#[tauri::command]
fn open_deep_link(app: AppHandle, url: String) -> Result<(), String> {
    handle_deep_link(&app, &url, "command")
}

#[tauri::command]
fn delete_games(app: AppHandle, game_ids: Vec<String>) -> Result<DeleteGamesSummary, String> {
    let game_count = game_ids.len().to_string();
    let requested_ids = game_ids.join(",");
    logging::event(
        "games_delete_requested",
        &[("game_count", &game_count), ("game_ids", &requested_ids)],
    );

    match games::install::delete_games(&app, game_ids) {
        Ok(summary) => {
            let deleted_count = summary.deleted_games.to_string();
            logging::event(
                "games_delete_completed",
                &[("deleted_count", &deleted_count)],
            );
            Ok(summary)
        }
        Err(message) => {
            logging::event("games_delete_failed", &[("reason", message.as_str())]);
            Err(message)
        }
    }
}

#[tauri::command]
fn list_categories(app: AppHandle) -> Result<Vec<Category>, String> {
    categories::list_categories(&app)
}

#[tauri::command]
fn save_category(app: AppHandle, input: CategoryInput) -> Result<Category, String> {
    categories::save_category(&app, input)
}

#[tauri::command]
fn delete_category(app: AppHandle, category_id: String) -> Result<(), String> {
    categories::delete_category(&app, category_id)
}

#[tauri::command]
fn classify_games_by_categories(app: AppHandle) -> Result<ClassifyGamesSummary, String> {
    logging::event("games_classify_requested", &[]);
    let categories = categories::list_categories(&app)?;

    match games::install::classify_games_by_categories(&app, categories) {
        Ok(summary) => {
            let updated_count = summary.updated_games.to_string();
            let matched_count = summary.matched_games.to_string();
            logging::event(
                "games_classify_completed",
                &[
                    ("updated_count", &updated_count),
                    ("matched_count", &matched_count),
                ],
            );
            Ok(summary)
        }
        Err(message) => {
            logging::event("games_classify_failed", &[("reason", message.as_str())]);
            Err(message)
        }
    }
}

#[tauri::command]
fn scan_games_from_files(
    app: AppHandle,
    files: Vec<IncomingGameFile>,
) -> Result<ScanGamesSummary, String> {
    games::install::scan_games_from_files(&app, files)
}

#[tauri::command]
fn scan_games_from_paths(
    app: AppHandle,
    files: Vec<IncomingGamePath>,
) -> Result<ScanGamesSummary, String> {
    games::install::scan_games_from_paths(&app, files)
}

#[tauri::command]
fn install_games_from_files(
    app: AppHandle,
    files: Vec<IncomingGameFile>,
    game_ids: Vec<String>,
) -> Result<InstallGamesSummary, String> {
    games::install::install_games_from_files(&app, files, game_ids)
}

#[tauri::command]
fn install_games_from_paths(
    app: AppHandle,
    files: Vec<IncomingGamePath>,
    game_ids: Vec<String>,
) -> Result<InstallGamesSummary, String> {
    games::install::install_games_from_paths(&app, files, game_ids)
}

#[tauri::command]
fn scan_game_sources(app: AppHandle, source_dirs: Vec<String>) -> Result<ScanGamesSummary, String> {
    games::install::scan_game_sources(&app, source_dirs)
}

#[tauri::command]
fn install_game_sources(
    app: AppHandle,
    source_dirs: Vec<String>,
    game_ids: Vec<String>,
) -> Result<InstallGamesSummary, String> {
    games::install::install_game_sources(&app, source_dirs, game_ids)
}

#[tauri::command]
fn export_games_archive(app: AppHandle) -> Result<ExportGamesArchiveSummary, String> {
    logging::event("games_export_requested", &[]);

    match games::export::export_games_archive(&app) {
        Ok(summary) => {
            let exported_games = summary.exported_games.to_string();
            let archive_path = summary.archive_path.clone();
            logging::event(
                "games_export_completed",
                &[
                    ("exported_games", &exported_games),
                    ("archive_path", &archive_path),
                ],
            );
            Ok(summary)
        }
        Err(message) => {
            logging::event("games_export_failed", &[("reason", message.as_str())]);
            Err(message)
        }
    }
}

#[tauri::command]
fn import_games_archive(
    app: AppHandle,
    archive_path: String,
) -> Result<ImportGamesArchiveSummary, String> {
    logging::event(
        "games_import_archive_requested",
        &[("archive_path", archive_path.as_str())],
    );

    match games::import::import_games_archive(&app, archive_path) {
        Ok(summary) => {
            let imported_games = summary.imported_games.to_string();
            let skipped_games = summary.skipped_games.to_string();
            let archive_path = summary.archive_path.clone();
            logging::event(
                "games_import_archive_completed",
                &[
                    ("imported_games", &imported_games),
                    ("skipped_games", &skipped_games),
                    ("archive_path", &archive_path),
                ],
            );
            Ok(summary)
        }
        Err(message) => {
            logging::event(
                "games_import_archive_failed",
                &[("reason", message.as_str())],
            );
            Err(message)
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            logging::event(
                "single_instance_received",
                &[("argv_count", &argv.len().to_string())],
            );
            for url in deep_link::parser::extract_deep_link_args(&argv) {
                if let Err(message) = handle_deep_link(app, &url, "single-instance") {
                    log::warn!("failed to handle forwarded deep link {url}: {message}");
                }
            }
        }));
    }

    builder
        .manage(DeepLinkState::default())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            fit_main_window_to_screen_height(&app_handle);

            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    let url = url.to_string();
                    if let Err(message) = handle_deep_link(&app_handle, &url, "plugin") {
                        log::warn!("failed to handle runtime deep link {url}: {message}");
                    }
                }
            });

            if let Ok(Some(urls)) = app.deep_link().get_current() {
                let app_handle = app.handle().clone();
                for url in urls {
                    let url = url.to_string();
                    if let Err(message) = handle_deep_link(&app_handle, &url, "startup") {
                        log::warn!("failed to handle startup deep link {url}: {message}");
                    }
                }
            }

            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("ytasset", |ctx, request, responder| {
            responder.respond(protocol::game_protocol::response_for_request(
                ctx.app_handle(),
                request,
            ));
        })
        .invoke_handler(tauri::generate_handler![
            last_deep_link_status,
            list_games,
            get_app_diagnostics,
            open_deep_link,
            delete_games,
            list_categories,
            save_category,
            delete_category,
            classify_games_by_categories,
            scan_games_from_files,
            scan_games_from_paths,
            scan_game_sources,
            install_games_from_files,
            install_games_from_paths,
            install_game_sources,
            export_games_archive,
            import_games_archive
        ])
        .run(tauri::generate_context!())
        .expect("error while running YeuTre Game Launcher");
}
