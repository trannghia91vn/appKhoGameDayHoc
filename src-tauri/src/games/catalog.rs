use crate::deep_link::parser::validate_game_id;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameManifest {
    pub id: String,
    pub title: String,
    pub grade: String,
    pub category: String,
    pub version: u32,
    pub entry: String,
    pub is_installed: bool,
    #[serde(default)]
    pub file_count: usize,
    #[serde(default)]
    pub total_bytes: u64,
    #[serde(default)]
    pub updated_at: u64,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogCache {
    version: u32,
    games: Vec<GameManifest>,
}

const CATALOG_CACHE_VERSION: u32 = 1;

pub fn list_games<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<GameManifest>, String> {
    if let Some(games) = read_catalog_cache(app)? {
        return Ok(games);
    }

    rebuild_catalog_cache(app)
}

pub fn rebuild_catalog_cache<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<GameManifest>, String> {
    let mut games = list_installed_games(app)?;
    sort_games(&mut games);
    write_catalog_cache(app, &games)?;
    Ok(games)
}

fn sort_games(games: &mut [GameManifest]) {
    games.sort_by(|left, right| left.title.cmp(&right.title));
}

fn catalog_cache_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("catalog.json"))
        .map_err(|err| format!("Khong xac dinh duoc file catalog cache cua app: {err}"))
}

fn read_catalog_cache<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Vec<GameManifest>>, String> {
    let cache_path = catalog_cache_path(app)?;
    let content = match fs::read_to_string(&cache_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("Khong doc duoc catalog cache: {err}")),
    };

    let cache = match serde_json::from_str::<CatalogCache>(&content) {
        Ok(cache) if cache.version == CATALOG_CACHE_VERSION => cache,
        _ => return Ok(None),
    };

    if !catalog_cache_is_valid(app, &cache.games)? {
        return Ok(None);
    }

    let mut games = cache.games;
    sort_games(&mut games);
    Ok(Some(games))
}

fn write_catalog_cache<R: Runtime>(
    app: &AppHandle<R>,
    games: &[GameManifest],
) -> Result<(), String> {
    let cache_path = catalog_cache_path(app)?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Khong tao duoc thu muc catalog cache: {err}"))?;
    }

    let cache = CatalogCache {
        version: CATALOG_CACHE_VERSION,
        games: games.to_vec(),
    };
    let content = serde_json::to_vec_pretty(&cache)
        .map_err(|err| format!("Khong tao duoc catalog cache: {err}"))?;
    fs::write(cache_path, content).map_err(|err| format!("Khong ghi duoc catalog cache: {err}"))
}

fn catalog_cache_is_valid<R: Runtime>(
    app: &AppHandle<R>,
    games: &[GameManifest],
) -> Result<bool, String> {
    let games_dir = installed_games_dir(app)?;
    if !games_dir.exists() {
        return Ok(games.is_empty());
    }

    let installed_dir_count = fs::read_dir(&games_dir)
        .map_err(|err| format!("Khong doc duoc thu muc games da cai: {err}"))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .map(|file_type| file_type.is_dir())
                .unwrap_or(false)
        })
        .filter(|entry| validate_game_id(&entry.file_name().to_string_lossy()).is_ok())
        .count();

    if installed_dir_count != games.len() {
        return Ok(false);
    }

    for game in games {
        if validate_game_id(&game.id).is_err() || validate_resource_path(&game.entry).is_err() {
            return Ok(false);
        }
        let entry_path = games_dir.join(&game.id).join(&game.entry);
        if !entry_path.is_file() {
            return Ok(false);
        }
    }

    Ok(true)
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

    let stats = installed_dir_stats(game_dir).unwrap_or_default();

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
        file_count: stats.file_count,
        total_bytes: stats.total_bytes,
        updated_at: stats.updated_at,
    })
}

#[derive(Default)]
struct InstalledDirStats {
    file_count: usize,
    total_bytes: u64,
    updated_at: u64,
}

fn installed_dir_stats(game_dir: &Path) -> Result<InstalledDirStats, String> {
    let mut stats = InstalledDirStats::default();
    collect_installed_dir_stats(game_dir, &mut stats)?;
    Ok(stats)
}

fn collect_installed_dir_stats(
    current_dir: &Path,
    stats: &mut InstalledDirStats,
) -> Result<(), String> {
    for entry in fs::read_dir(current_dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let file_type = entry.file_type().map_err(|err| err.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_installed_dir_stats(&path, stats)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let metadata = entry.metadata().map_err(|err| err.to_string())?;
        stats.file_count += 1;
        stats.total_bytes = stats.total_bytes.saturating_add(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        stats.updated_at = stats.updated_at.max(modified);
    }
    Ok(())
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
