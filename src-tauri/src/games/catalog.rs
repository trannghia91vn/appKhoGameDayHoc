use crate::deep_link::parser::validate_game_id;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameManifest {
    pub id: String,
    pub title: String,
    pub grade: String,
    pub category: String,
    pub version: u32,
    pub entry: String,
    pub is_installed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameMetadata {
    title: Option<String>,
    grade: Option<String>,
    category: Option<String>,
    version: Option<u32>,
    entry: Option<String>,
}

pub fn list_games<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<GameManifest>, String> {
    let mut games = list_installed_games(app)?;
    games.sort_by(|left, right| left.title.cmp(&right.title));
    Ok(games)
}

pub fn find_game<R: Runtime>(
    app: &AppHandle<R>,
    game_id: &str,
) -> Result<Option<GameManifest>, String> {
    Ok(list_games(app)?.into_iter().find(|game| game.id == game_id))
}

pub fn installed_games_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("games"))
        .map_err(|err| format!("Khong xac dinh duoc thu muc games cua app: {err}"))
}

pub fn read_installed_resource<R: Runtime>(
    app: &AppHandle<R>,
    game_id: &str,
    resource_path: &str,
) -> Result<Option<Vec<u8>>, String> {
    validate_game_id(game_id)?;
    validate_resource_path(resource_path)?;

    let game_dir = installed_games_dir(app)?.join(game_id);
    let mut path = game_dir.clone();
    for part in resource_path.split('/') {
        path.push(part);
    }

    if !path.starts_with(&game_dir) {
        return Err("Duong dan tai nguyen game khong hop le.".to_string());
    }

    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("Khong doc duoc tai nguyen game: {err}")),
    }
}

fn list_installed_games<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<GameManifest>, String> {
    let games_dir = installed_games_dir(app)?;
    if !games_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&games_dir)
        .map_err(|err| format!("Khong doc duoc thu muc games da cai: {err}"))?;
    let mut games = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|err| format!("Khong doc duoc game da cai: {err}"))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("Khong doc duoc loai file game: {err}"))?;
        if !file_type.is_dir() {
            continue;
        }

        let game_id = entry.file_name().to_string_lossy().to_string();
        if validate_game_id(&game_id).is_err() {
            continue;
        }

        if let Some(game) = manifest_from_installed_dir(&game_id, &entry.path()) {
            games.push(game);
        }
    }

    Ok(games)
}

fn manifest_from_installed_dir(game_id: &str, game_dir: &Path) -> Option<GameManifest> {
    let metadata = fs::read_to_string(game_dir.join("game.json"))
        .ok()
        .and_then(|content| serde_json::from_str::<GameMetadata>(&content).ok());

    let entry = metadata
        .as_ref()
        .and_then(|data| data.entry.as_deref())
        .filter(|value| validate_resource_path(value).is_ok())
        .unwrap_or("index.html")
        .to_string();

    if !game_dir.join(&entry).is_file() {
        return None;
    }

    Some(GameManifest {
        id: game_id.to_string(),
        title: metadata
            .as_ref()
            .and_then(|data| clean_text(data.title.as_deref()))
            .unwrap_or_else(|| title_from_id(game_id)),
        grade: metadata
            .as_ref()
            .and_then(|data| clean_text(data.grade.as_deref()))
            .unwrap_or_else(|| "Tuy chon".to_string()),
        category: metadata
            .as_ref()
            .and_then(|data| clean_text(data.category.as_deref()))
            .unwrap_or_else(|| "Game".to_string()),
        version: metadata.as_ref().and_then(|data| data.version).unwrap_or(1),
        entry,
        is_installed: true,
    })
}

fn clean_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn title_from_id(game_id: &str) -> String {
    game_id
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_resource_path(resource_path: &str) -> Result<(), String> {
    if resource_path.is_empty()
        || resource_path.starts_with('/')
        || resource_path.contains('\\')
        || resource_path
            .split('/')
            .any(|part| part == "." || part == ".." || part.is_empty())
    {
        return Err("Duong dan tai nguyen game khong hop le.".to_string());
    }

    Ok(())
}
