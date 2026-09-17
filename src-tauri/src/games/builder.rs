use crate::{deep_link::parser::validate_game_id, games::catalog};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabPair {
    pub id: String,
    pub image_path: String,
    pub answer: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabLesson {
    pub id: String,
    pub name: String,
    pub pairs: Vec<VocabPair>,
    pub wrongs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabGameDraft {
    pub id: String,
    pub title: String,
    pub lessons: Vec<VocabLesson>,
    pub updated_at: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuilderAsset {
    pub image_path: String,
}

fn drafts_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("builder/drafts"))
        .map_err(|e| e.to_string())
}
fn draft_dir<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<PathBuf, String> {
    validate_game_id(id)?;
    Ok(drafts_dir(app)?.join(id))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn draft_file(dir: &Path) -> PathBuf {
    dir.join("draft.json")
}
fn read_draft(dir: &Path) -> Result<VocabGameDraft, String> {
    serde_json::from_slice(
        &fs::read(draft_file(dir)).map_err(|e| format!("Không đọc được bản nháp: {e}"))?,
    )
    .map_err(|e| format!("Bản nháp không hợp lệ: {e}"))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        e.to_string()
    })
}
fn asset_name(path: &str) -> Result<&str, String> {
    if !path.starts_with("assets/") {
        return Err("Đường dẫn ảnh không hợp lệ.".into());
    }
    let name = &path[7..];
    if name.is_empty()
        || name.len() > 100
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || name.starts_with('.')
    {
        return Err("Tên ảnh không hợp lệ.".into());
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    if !matches!(ext, "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return Err("Chỉ nhận ảnh PNG, JPG, WEBP hoặc GIF.".into());
    }
    Ok(name)
}
fn validate_draft(draft: &VocabGameDraft, dir: &Path) -> Result<(), String> {
    validate_game_id(&draft.id)?;
    if draft.title.trim().is_empty() || draft.title.chars().count() > 100 {
        return Err("Tên game phải có 1-100 ký tự.".into());
    }
    if draft.lessons.len() > 100 {
        return Err("Tối đa 100 bài học.".into());
    }
    let mut images = 0usize;
    for lesson in &draft.lessons {
        if lesson.name.trim().is_empty() || lesson.name.chars().count() > 100 {
            return Err("Tên bài học phải có 1-100 ký tự.".into());
        }
        if lesson.pairs.len() > 100 || lesson.wrongs.len() > 200 {
            return Err("Bài học có quá nhiều từ.".into());
        }
        for pair in &lesson.pairs {
            if pair.answer.trim().is_empty() || pair.answer.chars().count() > 100 {
                return Err("Từ đáp án phải có 1-100 ký tự.".into());
            }
            asset_name(&pair.image_path)?;
            if !dir.join(&pair.image_path).is_file() {
                return Err(format!("Thiếu ảnh: {}", pair.image_path));
            }
            images += 1;
        }
        if lesson
            .wrongs
            .iter()
            .any(|w| w.trim().is_empty() || w.chars().count() > 100)
        {
            return Err("Từ gây nhiễu phải có 1-100 ký tự.".into());
        }
    }
    if images > 500 {
        return Err("Tối đa 500 ảnh cho mỗi game.".into());
    }
    Ok(())
}
pub fn list_drafts<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<VocabGameDraft>, String> {
    let root = drafts_dir(app)?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut drafts = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            if let Ok(draft) = read_draft(&entry.path()) {
                drafts.push(draft);
            }
        }
    }
    drafts.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(drafts)
}
pub fn create_draft<R: Runtime>(
    app: &AppHandle<R>,
    title: String,
) -> Result<VocabGameDraft, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 100 {
        return Err("Tên game phải có 1-100 ký tự.".into());
    }
    let id = format!("vocab-{}-{}", now(), std::process::id());
    let dir = draft_dir(app, &id)?;
    fs::create_dir_all(dir.join("assets")).map_err(|e| e.to_string())?;
    let draft = VocabGameDraft {
        id,
        title: title.to_owned(),
        lessons: vec![],
        updated_at: now(),
    };
    atomic_write(
        &draft_file(&dir),
        &serde_json::to_vec_pretty(&draft).map_err(|e| e.to_string())?,
    )?;
    Ok(draft)
}
pub fn save_draft<R: Runtime>(
    app: &AppHandle<R>,
    mut draft: VocabGameDraft,
) -> Result<VocabGameDraft, String> {
    let dir = draft_dir(app, &draft.id)?;
    if !draft_file(&dir).is_file() {
        return Err("Không tìm thấy bản nháp.".into());
    }
    validate_draft(&draft, &dir)?;
    draft.updated_at = now();
    atomic_write(
        &draft_file(&dir),
        &serde_json::to_vec_pretty(&draft).map_err(|e| e.to_string())?,
    )?;
    Ok(draft)
}
pub fn delete_draft<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), String> {
    let dir = draft_dir(app, id)?;
    if !draft_file(&dir).is_file() {
        return Err("Không tìm thấy bản nháp.".into());
    }
    fs::remove_dir_all(dir).map_err(|e| e.to_string())
}
pub fn add_image<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    source: &str,
) -> Result<BuilderAsset, String> {
    let dir = draft_dir(app, id)?;
    if !draft_file(&dir).is_file() {
        return Err("Không tìm thấy bản nháp.".into());
    }
    let path = Path::new(source);
    let bytes = fs::read(path).map_err(|e| format!("Không đọc được ảnh: {e}"))?;
    if bytes.len() > 3 * 1024 * 1024 {
        return Err("Ảnh vượt quá 3 MB.".into());
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return Err("Định dạng ảnh không hỗ trợ.".into());
    }
    let signature_ok = match ext.as_str() {
        "png" => bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]),
        "jpg" | "jpeg" => bytes.starts_with(&[255, 216, 255]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => false,
    };
    if !signature_ok {
        return Err("Nội dung ảnh không khớp định dạng.".into());
    }
    let image_path = format!("assets/{}-{}.{}", now(), std::process::id(), ext);
    let mut image_path = image_path;
    let mut counter = 0;
    while dir.join(&image_path).exists() {
        counter += 1;
        image_path = format!(
            "assets/{}-{}-{}.{}",
            now(),
            std::process::id(),
            counter,
            ext
        );
    }
    fs::write(dir.join(&image_path), bytes).map_err(|e| e.to_string())?;
    Ok(BuilderAsset { image_path })
}
pub fn get_image<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    image_path: &str,
) -> Result<Vec<u8>, String> {
    asset_name(image_path)?;
    fs::read(draft_dir(app, id)?.join(image_path)).map_err(|e| e.to_string())
}
pub fn export_game<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<String, String> {
    let dir = draft_dir(app, id)?;
    let draft = read_draft(&dir)?;
    validate_draft(&draft, &dir)?;
    if draft.lessons.is_empty() || draft.lessons.iter().any(|l| l.pairs.is_empty()) {
        return Err("Mỗi bài học cần ít nhất một cặp ảnh và từ.".into());
    }
    let game_id = draft.id.clone();
    let games_dir = catalog::installed_games_dir(app)?;
    fs::create_dir_all(&games_dir).map_err(|e| e.to_string())?;
    let target = games_dir.join(&game_id);
    if target.exists() {
        return Err("Game này đã có trong kho. Hãy xóa game cũ trước khi xuất lại.".into());
    }
    let stage = games_dir.join(format!(".builder-{}-{}", game_id, std::process::id()));
    fs::create_dir_all(stage.join("assets")).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        fs::create_dir_all(stage.join("data")).map_err(|e| e.to_string())?;
        let html = include_str!("vocab_player.html");
        fs::write(stage.join("index.html"), html).map_err(|e| e.to_string())?;
        fs::write(stage.join("game.json"), serde_json::to_vec_pretty(&serde_json::json!({"title":draft.title,"grade":"Tất cả","category":"Từ vựng","version":1,"entry":"index.html"})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        fs::write(
            stage.join("data/lessons.json"),
            serde_json::to_vec(&draft.lessons).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        for lesson in &draft.lessons {
            for pair in &lesson.pairs {
                let source = dir.join(&pair.image_path);
                fs::copy(&source, stage.join(&pair.image_path)).map_err(|e| e.to_string())?;
            }
        }
        fs::rename(&stage, &target).map_err(|e| e.to_string())?;
        catalog::rebuild_catalog_cache(app)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    result?;
    Ok(game_id)
}

const MAX_GENERATED_HTML_BYTES: usize = 80 * 1024 * 1024;
const GENERATED_GAME_ID: &str = "game-tu-vung";

fn is_vocab_builder_html(bytes: &[u8]) -> bool {
    let Ok(html) = std::str::from_utf8(bytes) else {
        return false;
    };
    html.contains("vocab_drag_lessons_v1")
        && html.contains("studentModeFlag")
        && html.contains("Kho Từ Vựng")
}

fn find_existing_generated_game<R: Runtime>(app: &AppHandle<R>) -> Result<Option<String>, String> {
    let games_dir = catalog::installed_games_dir(app)?;
    let mut candidates = catalog::list_games(app)?
        .into_iter()
        .filter_map(|game| {
            let entry = games_dir.join(&game.id).join(&game.entry);
            let bytes = fs::read(entry).ok()?;
            is_vocab_builder_html(&bytes).then_some((game.updated_at, game.id))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(candidates.into_iter().next().map(|(_, id)| id))
}

pub fn upsert_generated_html_game<R: Runtime>(
    app: &AppHandle<R>,
    title: String,
    html: Vec<u8>,
) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 100 {
        return Err("Tên game phải có 1-100 ký tự.".into());
    }
    if html.is_empty() || html.len() > MAX_GENERATED_HTML_BYTES || !is_vocab_builder_html(&html) {
        return Err("File game từ vựng xuất ra không hợp lệ.".into());
    }

    let game_id =
        find_existing_generated_game(app)?.unwrap_or_else(|| GENERATED_GAME_ID.to_string());
    validate_game_id(&game_id)?;

    let games_dir = catalog::installed_games_dir(app)?;
    fs::create_dir_all(&games_dir).map_err(|err| err.to_string())?;
    let target = games_dir.join(&game_id);
    let stage = games_dir.join(format!(".vocab-stage-{}-{}", game_id, std::process::id()));
    let backup = games_dir.join(format!(".vocab-backup-{}-{}", game_id, std::process::id()));
    let _ = fs::remove_dir_all(&stage);
    let _ = fs::remove_dir_all(&backup);
    fs::create_dir_all(&stage).map_err(|err| format!("Không tạo được vùng tạm: {err}"))?;

    let result = (|| -> Result<(), String> {
        fs::write(stage.join("index.html"), html)
            .map_err(|err| format!("Không ghi được game HTML: {err}"))?;
        let metadata = serde_json::json!({
            "title": title,
            "grade": "Tùy chọn",
            "category": "Từ vựng",
            "version": now() as u32,
            "entry": "index.html",
            "source": "vocab-builder"
        });
        fs::write(
            stage.join("game.json"),
            serde_json::to_vec_pretty(&metadata).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("Không ghi được metadata game: {err}"))?;

        if target.exists() {
            fs::rename(&target, &backup)
                .map_err(|err| format!("Không tạo được bản sao an toàn: {err}"))?;
        }
        if let Err(err) = fs::rename(&stage, &target) {
            if backup.exists() {
                let _ = fs::rename(&backup, &target);
            }
            return Err(format!("Không cập nhật được game: {err}"));
        }
        let _ = fs::remove_dir_all(&backup);
        catalog::rebuild_catalog_cache(app)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&stage);
        if backup.exists() && !target.exists() {
            let _ = fs::rename(&backup, &target);
        }
    }
    result?;
    Ok(game_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_asset_paths() {
        assert!(asset_name("assets/a.png").is_ok());
        assert!(asset_name("../a.png").is_err());
        assert!(asset_name("assets/../a.png").is_err());
        assert!(asset_name("assets/a\\b.png").is_err());
    }

    #[test]
    fn recognizes_exported_vocab_builder_html() {
        assert!(is_vocab_builder_html(
            r#"<title>Kho Từ Vựng</title><script>const LS_LESSONS = "vocab_drag_lessons_v1";</script><script id="studentModeFlag"></script>"#.as_bytes()
        ));
        assert!(!is_vocab_builder_html(
            b"<html><title>Other game</title></html>"
        ));
    }
}
