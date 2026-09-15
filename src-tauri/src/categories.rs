use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::PathBuf};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub category_id: Option<String>,
    pub name: String,
    pub keywords: Vec<String>,
}

pub fn list_categories<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Category>, String> {
    let path = categories_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Khong doc duoc danh sach categories: {err}"))?;
    let mut categories = serde_json::from_str::<Vec<Category>>(&content)
        .map_err(|err| format!("Danh sach categories khong hop le: {err}"))?;
    categories.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(categories)
}

pub fn save_category<R: Runtime>(
    app: &AppHandle<R>,
    input: CategoryInput,
) -> Result<Category, String> {
    let name = clean_name(&input.name)?;
    let keywords = clean_keywords(input.keywords)?;
    let mut categories = list_categories(app)?;

    if categories.iter().any(|category| {
        category.id != input.category_id.clone().unwrap_or_default()
            && category.name.eq_ignore_ascii_case(&name)
    }) {
        return Err("Ten category da ton tai.".to_string());
    }

    let category = if let Some(category_id) = input.category_id {
        let category = categories
            .iter_mut()
            .find(|category| category.id == category_id)
            .ok_or_else(|| "Khong tim thay category can sua.".to_string())?;
        category.name = name;
        category.keywords = keywords;
        category.clone()
    } else {
        let id = category_id_from_name(&name)?;
        if categories.iter().any(|category| category.id == id) {
            return Err("Category co cung ma ID da ton tai.".to_string());
        }
        let category = Category { id, name, keywords };
        categories.push(category.clone());
        category
    };

    write_categories(app, &categories)?;
    Ok(category)
}

pub fn delete_category<R: Runtime>(app: &AppHandle<R>, category_id: String) -> Result<(), String> {
    let mut categories = list_categories(app)?;
    let original_len = categories.len();
    categories.retain(|category| category.id != category_id);

    if categories.len() == original_len {
        return Err("Khong tim thay category can xoa.".to_string());
    }

    write_categories(app, &categories)
}

fn categories_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("categories.json"))
        .map_err(|err| format!("Khong xac dinh duoc file categories cua app: {err}"))
}

fn write_categories<R: Runtime>(app: &AppHandle<R>, categories: &[Category]) -> Result<(), String> {
    let path = categories_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Khong tao duoc thu muc du lieu categories: {err}"))?;
    }

    let content = serde_json::to_vec_pretty(categories)
        .map_err(|err| format!("Khong tao duoc noi dung categories: {err}"))?;
    fs::write(path, content).map_err(|err| format!("Khong luu duoc categories: {err}"))
}

fn clean_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Ten category khong duoc de trong.".to_string());
    }
    if value.chars().count() > 100 {
        return Err("Ten category toi da 100 ky tu.".to_string());
    }
    Ok(value.to_string())
}

fn clean_keywords(values: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    let mut keywords = Vec::new();

    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > 80 {
            return Err("Moi keyword toi da 80 ky tu.".to_string());
        }
        let key = value.to_lowercase();
        if seen.insert(key) {
            keywords.push(value.to_string());
        }
    }

    if keywords.is_empty() {
        return Err("Hay nhap it nhat mot keyword.".to_string());
    }

    Ok(keywords)
}

fn category_id_from_name(name: &str) -> Result<String, String> {
    let mut id = String::new();
    let mut previous_was_separator = false;

    for character in name.to_lowercase().chars() {
        if let Some(ascii) = vietnamese_ascii_fold(character) {
            id.push(ascii);
            previous_was_separator = false;
        } else if character.is_ascii_alphanumeric() {
            id.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator && !id.is_empty() {
            id.push('-');
            previous_was_separator = true;
        }
    }

    let id = id
        .chars()
        .take(80)
        .collect::<String>()
        .trim_end_matches('-')
        .to_string();
    if id.is_empty() {
        return Err("Ten category phai co chu hoac so de tao ID.".to_string());
    }

    Ok(id)
}

fn vietnamese_ascii_fold(character: char) -> Option<char> {
    match character {
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

#[cfg(test)]
mod tests {
    use super::{category_id_from_name, clean_keywords};

    #[test]
    fn creates_stable_category_id() {
        assert_eq!(category_id_from_name("Toán lớp 4").unwrap(), "toan-lop-4");
    }

    #[test]
    fn trims_and_deduplicates_keywords() {
        assert_eq!(
            clean_keywords(vec![
                "Toán".to_string(),
                " Toán ".to_string(),
                "cộng".to_string()
            ])
            .unwrap(),
            vec!["Toán".to_string(), "cộng".to_string()]
        );
    }
}
