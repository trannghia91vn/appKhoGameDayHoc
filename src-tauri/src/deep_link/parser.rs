use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLinkCommand {
    Play { game_id: String },
}

pub fn parse_deep_link(input: &str) -> Result<DeepLinkCommand, String> {
    if input.contains("..") || input.contains('\\') {
        return Err("Deep link khong duoc chua path traversal.".to_string());
    }

    let url = Url::parse(input).map_err(|_| "Deep link khong dung dinh dang URL.".to_string())?;

    if url.scheme() != "yeutregame" {
        return Err("Deep link phai bat dau bang yeutregame://.".to_string());
    }

    if url.fragment().is_some() {
        return Err("Deep link khong duoc co fragment.".to_string());
    }

    let command = url
        .host_str()
        .ok_or_else(|| "Deep link thieu lenh play.".to_string())?;

    if command != "play" {
        return Err("PoC chi ho tro yeutregame://play/<game-id>.".to_string());
    }

    let segments = url
        .path_segments()
        .ok_or_else(|| "Deep link thieu game ID.".to_string())?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.len() != 1 {
        return Err("Deep link play chi duoc truyen mot game ID.".to_string());
    }

    let game_id = segments[0];
    validate_game_id(game_id)?;

    Ok(DeepLinkCommand::Play {
        game_id: game_id.to_string(),
    })
}

pub fn extract_deep_link_args(args: &[String]) -> Vec<String> {
    args.iter()
        .filter(|arg| arg.starts_with("yeutregame://"))
        .cloned()
        .collect()
}

pub fn validate_game_id(game_id: &str) -> Result<(), String> {
    if game_id.is_empty() {
        return Err("Game ID khong duoc de trong.".to_string());
    }

    if game_id.len() > 80 {
        return Err("Game ID qua dai.".to_string());
    }

    let valid = game_id
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_');

    if !valid {
        return Err("Game ID chi duoc gom a-z, 0-9, dau gach ngang va gach duoi.".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_deep_link, DeepLinkCommand};

    #[test]
    fn accepts_play_link() {
        assert_eq!(
            parse_deep_link("yeutregame://play/toan-lop-4").unwrap(),
            DeepLinkCommand::Play {
                game_id: "toan-lop-4".to_string()
            }
        );
    }

    #[test]
    fn accepts_powerpoint_query() {
        assert_eq!(
            parse_deep_link("yeutregame://play/toan-lop-4?OR=PowerPoint").unwrap(),
            DeepLinkCommand::Play {
                game_id: "toan-lop-4".to_string()
            }
        );
    }

    #[test]
    fn rejects_invalid_game_id() {
        assert!(parse_deep_link("yeutregame://play/Toan-Lop-4").is_err());
    }

    #[test]
    fn rejects_traversal() {
        assert!(parse_deep_link("yeutregame://play/../../secret").is_err());
    }

    #[test]
    fn rejects_nested_url() {
        assert!(parse_deep_link("yeutregame://play/http://x").is_err());
    }
}
