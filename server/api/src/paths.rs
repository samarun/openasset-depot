use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

pub fn normalize_depot_path(input: &str) -> AppResult<String> {
    if input.trim().is_empty() {
        return Err(AppError::bad_request("path cannot be empty"));
    }
    if input.contains('\0') {
        return Err(AppError::bad_request("path contains a null byte"));
    }
    if input.contains('\\') {
        return Err(AppError::bad_request("use forward slashes in depot paths"));
    }
    if input.starts_with('/') {
        return Err(AppError::bad_request("depot paths must be relative"));
    }
    if input.len() > 4_096 {
        return Err(AppError::bad_request("depot path exceeds 4096 bytes"));
    }

    let path = Path::new(input);
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let segment = value
                    .to_str()
                    .ok_or_else(|| AppError::bad_request("path must be valid UTF-8"))?;
                if segment.is_empty() {
                    continue;
                }
                validate_portable_segment(segment)?;
                parts.push(segment.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(AppError::bad_request("path traversal is not allowed"));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::bad_request("absolute paths are not allowed"));
            }
        }
    }

    if parts.is_empty() {
        return Err(AppError::bad_request("path cannot be empty"));
    }

    Ok(parts.join("/"))
}

fn validate_portable_segment(segment: &str) -> AppResult<()> {
    if segment.len() > 255 {
        return Err(AppError::bad_request("path segment exceeds 255 bytes"));
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(AppError::bad_request(
            "path segments cannot end with a dot or space",
        ));
    }
    if segment
        .chars()
        .any(|character| character.is_control() || r#"<>:"|?*"#.contains(character))
    {
        return Err(AppError::bad_request(
            "path contains characters that are not portable across studio platforms",
        ));
    }
    let basename = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_uppercase();
    let reserved = matches!(basename.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || basename
            .strip_prefix("COM")
            .or_else(|| basename.strip_prefix("LPT"))
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|number| (1..=9).contains(&number));
    if reserved {
        return Err(AppError::bad_request(
            "path uses a reserved Windows device name",
        ));
    }
    Ok(())
}

pub fn safe_join(root: &Path, depot_path: &str) -> AppResult<PathBuf> {
    let normalized = normalize_depot_path(depot_path)?;
    Ok(root.join(normalized))
}

#[cfg(test)]
mod tests {
    use super::normalize_depot_path;

    #[test]
    fn normalizes_simple_paths() {
        assert_eq!(
            normalize_depot_path("./Content/Maps/Main.umap").unwrap(),
            "Content/Maps/Main.umap"
        );
    }

    #[test]
    fn rejects_traversal() {
        assert!(normalize_depot_path("../Secrets.uasset").is_err());
        assert!(normalize_depot_path("Content/../Secrets.uasset").is_err());
        assert!(normalize_depot_path("/absolute/file.uasset").is_err());
        assert!(normalize_depot_path("Content\\file.uasset").is_err());
        assert!(normalize_depot_path("Content/CON.txt").is_err());
        assert!(normalize_depot_path("Content/shot?.blend").is_err());
        assert!(normalize_depot_path("Content/trailing. ").is_err());
    }
}
