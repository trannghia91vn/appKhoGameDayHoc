use crate::{categories::Category, deep_link::parser::validate_game_id, games::catalog};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};
use tauri::{AppHandle, Runtime};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingGameFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallGamesSummary {
    pub copied_files: usize,
    pub installed_games: usize,
    pub skipped_files: usize,
    pub target_dir: String,
    pub game_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteGamesSummary {
    pub deleted_games: usize,
    pub skipped_games: usize,
    pub game_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredGame {
    pub id: String,
    pub title: String,
    pub grade: String,
    pub category: String,
    pub version: u32,
    pub entry: String,
    pub file_count: usize,
    pub is_new: bool,
    pub source_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanGamesSummary {
    pub games: Vec<DiscoveredGame>,
    pub skipped_files: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedGame {
    pub id: String,
    pub title: String,
    pub previous_category: String,
    pub category: String,
    pub matched_keywords: Vec<String>,
    pub updated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyGamesSummary {
    pub scanned_games: usize,
    pub matched_games: usize,
    pub updated_games: usize,
    pub unchanged_games: usize,
    pub skipped_games: usize,
    pub games: Vec<ClassifiedGame>,
}

struct HtmlGameSource {
    title: String,
    source_path: String,
    bytes: Vec<u8>,
}

pub fn delete_games<R: Runtime>(
    app: &AppHandle<R>,
    game_ids: Vec<String>,
) -> Result<DeleteGamesSummary, String> {
    if game_ids.is_empty() {
        return Err("Chua chon game nao de xoa.".to_string());
    }

    let target_root = catalog::installed_games_dir(app)?;
    let mut deleted_ids = Vec::new();
    let mut skipped_games = 0;

    for game_id in game_ids {
        validate_game_id(&game_id)?;
        let target_dir = target_root.join(&game_id);

        if !target_dir.is_dir() {
            skipped_games += 1;
            continue;
        }

        fs::remove_dir_all(&target_dir)
            .map_err(|err| format!("Khong xoa duoc game {game_id}: {err}"))?;
        deleted_ids.push(game_id);
    }

    if deleted_ids.is_empty() {
        return Err("Khong co game da cai nao trong danh sach de xoa.".to_string());
    }

    Ok(DeleteGamesSummary {
        deleted_games: deleted_ids.len(),
        skipped_games,
        game_ids: deleted_ids,
    })
}

pub fn scan_games_from_files<R: Runtime>(
    app: &AppHandle<R>,
    files: Vec<IncomingGameFile>,
) -> Result<ScanGamesSummary, String> {
    if files.is_empty() {
        return Err("Chua co file HTML nao de quet.".to_string());
    }

    let (html_files, skipped_files) = collect_html_game_files(files);
    if html_files.is_empty() {
        return Err("Khong tim thay file .html hop le trong folder da chon.".to_string());
    }

    let installed_ids = catalog::list_games(app)?
        .into_iter()
        .map(|game| game.id)
        .collect::<HashSet<_>>();

    let games = html_files
        .into_iter()
        .map(|(game_id, source)| DiscoveredGame {
            id: game_id.clone(),
            title: source.title,
            grade: "Tuy chon".to_string(),
            category: "HTML".to_string(),
            version: 1,
            entry: "index.html".to_string(),
            file_count: 1,
            is_new: !installed_ids.contains(&game_id),
            source_path: source.source_path,
        })
        .collect::<Vec<_>>();

    Ok(ScanGamesSummary {
        games,
        skipped_files,
    })
}

pub fn install_games_from_files<R: Runtime>(
    app: &AppHandle<R>,
    files: Vec<IncomingGameFile>,
    game_ids: Vec<String>,
) -> Result<InstallGamesSummary, String> {
    if game_ids.is_empty() {
        return Err("Khong co file HTML moi nao de cap nhat.".to_string());
    }

    let (mut html_files, mut skipped_files) = collect_html_game_files(files);
    let selected_ids = game_ids.into_iter().collect::<HashSet<_>>();
    html_files.retain(|game_id, _| selected_ids.contains(game_id));

    if html_files.is_empty() {
        return Err("Khong tim thay cac file HTML da xac nhan trong folder da chon.".to_string());
    }

    let installed_ids = catalog::list_games(app)?
        .into_iter()
        .map(|game| game.id)
        .collect::<HashSet<_>>();
    let target_root = catalog::installed_games_dir(app)?;
    fs::create_dir_all(&target_root)
        .map_err(|err| format!("Khong tao duoc thu muc games trong app: {err}"))?;

    let mut copied_files = 0;
    let mut installed_game_ids = Vec::new();

    for (game_id, source) in html_files {
        if installed_ids.contains(&game_id) {
            skipped_files += 1;
            continue;
        }

        validate_game_id(&game_id)?;
        let target_dir = target_root.join(&game_id);
        fs::create_dir_all(&target_dir)
            .map_err(|err| format!("Khong tao duoc thu muc game {game_id}: {err}"))?;

        let target_html_path = target_dir.join("index.html");
        ensure_child_path(&target_dir, &target_html_path)?;
        fs::write(&target_html_path, source.bytes)
            .map_err(|err| format!("Khong chep duoc file HTML {game_id}: {err}"))?;

        let metadata = serde_json::json!({
            "title": source.title,
            "grade": "Tuy chon",
            "category": "HTML",
            "version": 1,
            "entry": "index.html"
        });
        let metadata_bytes = serde_json::to_vec_pretty(&metadata)
            .map_err(|err| format!("Khong tao duoc metadata cho {game_id}: {err}"))?;
        let metadata_path = target_dir.join("game.json");
        ensure_child_path(&target_dir, &metadata_path)?;
        fs::write(&metadata_path, metadata_bytes)
            .map_err(|err| format!("Khong ghi duoc metadata cho {game_id}: {err}"))?;

        copied_files += 1;
        installed_game_ids.push(game_id);
    }

    if installed_game_ids.is_empty() {
        return Err("Khong co file HTML moi nao duoc cap nhat.".to_string());
    }

    Ok(InstallGamesSummary {
        copied_files,
        installed_games: installed_game_ids.len(),
        skipped_files,
        target_dir: target_root.to_string_lossy().to_string(),
        game_ids: installed_game_ids,
    })
}

pub fn classify_games_by_categories<R: Runtime>(
    app: &AppHandle<R>,
    categories: Vec<Category>,
) -> Result<ClassifyGamesSummary, String> {
    if categories.is_empty() {
        return Err("Hay tao it nhat mot category truoc khi phan loai game.".to_string());
    }

    let installed_games = catalog::list_games(app)?
        .into_iter()
        .filter(|game| game.is_installed)
        .collect::<Vec<_>>();

    if installed_games.is_empty() {
        return Err("Chua co game da cai nao de phan loai.".to_string());
    }

    let category_rules = categories
        .into_iter()
        .map(|category| {
            let keywords = category
                .keywords
                .iter()
                .filter_map(|keyword| {
                    normalize_search_text(keyword).map(|normalized| (keyword.clone(), normalized))
                })
                .collect::<Vec<_>>();
            (category, keywords)
        })
        .filter(|(_, keywords)| !keywords.is_empty())
        .collect::<Vec<_>>();

    if category_rules.is_empty() {
        return Err("Cac category hien tai chua co keyword hop le de phan loai.".to_string());
    }

    let games_dir = catalog::installed_games_dir(app)?;
    let mut classified_games = Vec::new();
    let mut skipped_games = 0;

    for game in installed_games {
        let Some(search_text) = normalize_search_text(&format!("{} {}", game.title, game.id))
        else {
            skipped_games += 1;
            continue;
        };

        let Some((category, matched_keywords)) = best_category_match(&search_text, &category_rules)
        else {
            skipped_games += 1;
            continue;
        };

        let updated = game.category != category.name;
        if updated {
            let game_dir = games_dir.join(&game.id);
            let metadata_path = game_dir.join("game.json");
            ensure_child_path(&game_dir, &metadata_path)?;
            write_classified_metadata(&metadata_path, &game, &category.name)?;
        }

        classified_games.push(ClassifiedGame {
            id: game.id,
            title: game.title,
            previous_category: game.category,
            category: category.name.clone(),
            matched_keywords,
            updated,
        });
    }

    let matched_games = classified_games.len();
    let updated_games = classified_games.iter().filter(|game| game.updated).count();

    Ok(ClassifyGamesSummary {
        scanned_games: matched_games + skipped_games,
        matched_games,
        updated_games,
        unchanged_games: matched_games.saturating_sub(updated_games),
        skipped_games,
        games: classified_games,
    })
}

fn best_category_match<'a>(
    search_text: &str,
    category_rules: &'a [(Category, Vec<(String, String)>)],
) -> Option<(&'a Category, Vec<String>)> {
    let mut best_match: Option<(&Category, Vec<String>)> = None;

    for (category, keywords) in category_rules {
        let matched_keywords = keywords
            .iter()
            .filter(|(_, normalized)| contains_normalized_keyword(search_text, normalized))
            .map(|(keyword, _)| keyword.clone())
            .collect::<Vec<_>>();

        if matched_keywords.is_empty() {
            continue;
        }

        let should_replace = best_match
            .as_ref()
            .map(|(_, current_keywords)| matched_keywords.len() > current_keywords.len())
            .unwrap_or(true);
        if should_replace {
            best_match = Some((category, matched_keywords));
        }
    }

    best_match
}

fn contains_normalized_keyword(search_text: &str, keyword: &str) -> bool {
    let haystack = format!(" {search_text} ");
    let needle = format!(" {keyword} ");
    haystack.contains(&needle)
}

fn write_classified_metadata(
    metadata_path: &Path,
    game: &catalog::GameManifest,
    category_name: &str,
) -> Result<(), String> {
    let mut metadata = fs::read_to_string(metadata_path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();

    metadata
        .entry("title".to_string())
        .or_insert_with(|| serde_json::Value::String(game.title.clone()));
    metadata
        .entry("grade".to_string())
        .or_insert_with(|| serde_json::Value::String(game.grade.clone()));
    metadata.insert(
        "category".to_string(),
        serde_json::Value::String(category_name.to_string()),
    );
    metadata
        .entry("version".to_string())
        .or_insert_with(|| serde_json::Value::from(game.version));
    metadata
        .entry("entry".to_string())
        .or_insert_with(|| serde_json::Value::String(game.entry.clone()));

    let content = serde_json::to_vec_pretty(&serde_json::Value::Object(metadata))
        .map_err(|err| format!("Khong tao duoc metadata phan loai cho {}: {err}", game.id))?;
    fs::write(metadata_path, content)
        .map_err(|err| format!("Khong ghi duoc category cho {}: {err}", game.id))
}

fn collect_html_game_files(
    files: Vec<IncomingGameFile>,
) -> (BTreeMap<String, HtmlGameSource>, usize) {
    let mut skipped_files = 0;
    let mut html_files = BTreeMap::new();

    for file in files {
        let Some(components) = normalize_relative_path(&file.relative_path) else {
            skipped_files += 1;
            continue;
        };

        if should_skip_file(&components) || !is_html_file(&components) {
            skipped_files += 1;
            continue;
        }

        let Some(file_name) = components.last() else {
            skipped_files += 1;
            continue;
        };
        let Some(stem) = file_stem(file_name) else {
            skipped_files += 1;
            continue;
        };
        let Some(game_id) = game_id_from_html_stem(stem) else {
            skipped_files += 1;
            continue;
        };

        if html_files.contains_key(&game_id) {
            skipped_files += 1;
            continue;
        }

        html_files.insert(
            game_id,
            HtmlGameSource {
                title: title_from_file_stem(stem),
                source_path: components.join("/"),
                bytes: file.bytes,
            },
        );
    }

    (html_files, skipped_files)
}

fn normalize_relative_path(path: &str) -> Option<Vec<String>> {
    if path.contains('\\') {
        return None;
    }

    let components = path
        .split('/')
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if components.is_empty()
        || components
            .iter()
            .any(|part| part == "." || part == ".." || part.contains('\0'))
    {
        return None;
    }

    Some(components)
}

fn should_skip_file(components: &[String]) -> bool {
    components.iter().any(|part| part == "__MACOSX")
        || components
            .last()
            .map(|name| name == ".DS_Store" || name == "Thumbs.db")
            .unwrap_or(false)
}

fn is_html_file(components: &[String]) -> bool {
    components
        .last()
        .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "html" | "htm"))
        .unwrap_or(false)
}

fn file_stem(file_name: &str) -> Option<&str> {
    file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem.trim())
        .filter(|stem| !stem.is_empty())
}

fn title_from_file_stem(stem: &str) -> String {
    stem.replace(['_', '-'], " ").trim().to_string()
}

fn game_id_from_html_stem(stem: &str) -> Option<String> {
    let mut id = String::new();
    let mut previous_was_separator = false;

    for ch in stem.to_lowercase().chars() {
        if let Some(ascii) = vietnamese_ascii_fold(ch) {
            id.push(ascii);
            previous_was_separator = false;
        } else if ch.is_ascii_alphanumeric() {
            id.push(ch);
            previous_was_separator = false;
        } else if !previous_was_separator && !id.is_empty() {
            id.push('-');
            previous_was_separator = true;
        }

        if id.len() >= 80 {
            break;
        }
    }

    while id.ends_with('-') {
        id.pop();
    }

    validate_game_id(&id).ok()?;
    Some(id)
}

fn normalize_search_text(value: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut previous_was_separator = true;

    for character in value.to_lowercase().chars() {
        if let Some(ascii) = vietnamese_ascii_fold(character) {
            normalized.push(ascii);
            previous_was_separator = false;
        } else if character.is_ascii_alphanumeric() {
            normalized.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push(' ');
            previous_was_separator = true;
        }
    }

    let normalized = normalized.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn vietnamese_ascii_fold(ch: char) -> Option<char> {
    match ch {
        'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ' | 'ẩ'
        | 'ẫ' | 'ậ' => Some('a'),
        'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => {
            Some('e')
        }
        'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => Some('i'),
        'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ' | 'ở'
        | 'ỡ' | 'ợ' => Some('o'),
        'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => {
            Some('u')
        }
        'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => Some('y'),
        'đ' => Some('d'),
        _ => None,
    }
}

fn ensure_child_path(parent: &Path, child: &Path) -> Result<(), String> {
    if child.starts_with(parent) {
        Ok(())
    } else {
        Err("Duong dan file game khong hop le.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        best_category_match, collect_html_game_files, game_id_from_html_stem,
        normalize_search_text, IncomingGameFile,
    };
    use crate::categories::Category;

    fn incoming(relative_path: &str) -> IncomingGameFile {
        IncomingGameFile {
            relative_path: relative_path.to_string(),
            bytes: vec![1, 2, 3],
        }
    }

    #[test]
    fn collects_html_files_as_independent_games() {
        let (games, skipped) = collect_html_game_files(vec![
            incoming("usb-games/Toan Lop 4.html"),
            incoming("usb-games/Tieng Viet 5.htm"),
            incoming("usb-games/readme.txt"),
        ]);

        assert!(games.contains_key("toan-lop-4"));
        assert!(games.contains_key("tieng-viet-5"));
        assert_eq!(games.len(), 2);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn folds_vietnamese_names_for_game_ids() {
        assert_eq!(
            game_id_from_html_stem("Toán lớp 4 - Đại lượng").unwrap(),
            "toan-lop-4-dai-luong"
        );
    }

    #[test]
    fn normalizes_vietnamese_text_for_category_matching() {
        assert_eq!(
            normalize_search_text("Học Toán cùng bé").unwrap(),
            "hoc toan cung be"
        );
    }

    #[test]
    fn matches_category_by_normalized_keywords() {
        let category = Category {
            id: "toan".to_string(),
            name: "Toán".to_string(),
            keywords: vec!["toán".to_string(), "cộng".to_string()],
        };
        let rules = vec![(
            category,
            vec![
                ("toán".to_string(), normalize_search_text("toán").unwrap()),
                ("cộng".to_string(), normalize_search_text("cộng").unwrap()),
            ],
        )];

        let (_, matched_keywords) =
            best_category_match("hoc toan cung be", &rules).expect("expected category match");
        assert_eq!(matched_keywords, vec!["toán".to_string()]);
    }

    #[test]
    fn ignores_invalid_and_duplicate_files() {
        let (games, skipped) = collect_html_game_files(vec![
            incoming("usb-games/Bài học.html"),
            incoming("usb-games/bai hoc.HTML"),
            incoming("usb-games/.DS_Store"),
            incoming("usb-games/../secret.html"),
        ]);

        assert_eq!(games.len(), 1);
        assert_eq!(skipped, 3);
    }
}
