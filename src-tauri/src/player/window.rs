use crate::{deep_link::parser::validate_game_id, games::catalog, logging};
use tauri::{Manager, WebviewUrl};
use url::Url;

pub fn open_game(app: &tauri::AppHandle, game_id: &str) -> Result<(), String> {
    validate_game_id(game_id)?;

    let game = catalog::find_game(app, game_id)?
        .ok_or_else(|| format!("Khong tim thay tro choi co ma: {game_id}"))?;

    logging::event(
        "game_open",
        &[
            ("game_id", game.id.as_str()),
            ("version", &game.version.to_string()),
        ],
    );

    let url = Url::parse(&format!("ytasset://game/{}/{}", game.id, game.entry))
        .map_err(|err| format!("Khong tao duoc player URL: {err}"))?;
    let label = format!("player-{}", game.id);

    if let Some(window) = app.get_webview_window(&label) {
        window
            .navigate(url)
            .map_err(|err| format!("Khong dieu huong player window: {err}"))?;
        window
            .show()
            .map_err(|err| format!("Khong hien player window: {err}"))?;
        window
            .set_focus()
            .map_err(|err| format!("Khong focus player window: {err}"))?;
        return Ok(());
    }

    let window = tauri::WebviewWindowBuilder::new(app, label, WebviewUrl::External(url))
        .title(format!("YeuTre Game - {}", game.title))
        .inner_size(1120.0, 820.0)
        .min_inner_size(860.0, 620.0)
        .resizable(true)
        .maximized(true)
        .build()
        .map_err(|err| format!("Khong tao duoc player window: {err}"))?;

    window
        .set_focus()
        .map_err(|err| format!("Khong focus player window: {err}"))?;

    Ok(())
}
