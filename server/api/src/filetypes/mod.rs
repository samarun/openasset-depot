use std::collections::HashMap;

use axum::{extract::State, http::HeaderMap, Json};
use globset::{Glob, GlobMatcher};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    api::AppState,
    error::{AppError, AppResult},
    paths::normalize_depot_path,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTypeRule {
    pub id: Uuid,
    pub name: String,
    pub priority: i32,
    pub rule_kind: String,
    pub exact_path: Option<String>,
    pub directory_prefix: Option<String>,
    pub extension: Option<String>,
    pub glob: Option<String>,
    pub regex: Option<String>,
    pub asset_class: String,
    pub is_binary: bool,
    pub lock_required: bool,
    pub large_file: bool,
    pub generated: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    rule: FileTypeRule,
    glob: Option<GlobMatcher>,
    regex: Option<Regex>,
}

#[derive(Debug, Clone)]
pub struct FileTypeMatcher {
    exact: HashMap<String, CompiledRule>,
    directories: Vec<CompiledRule>,
    extensions: HashMap<String, CompiledRule>,
    fallback: CompiledRule,
}

impl FileTypeMatcher {
    pub async fn load_from_db(db: &PgPool) -> AppResult<Self> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, priority, rule_kind, exact_path, directory_prefix, extension, glob, regex,
                   asset_class, is_binary, lock_required, large_file, generated, enabled
            FROM file_type_rules
            WHERE enabled = TRUE
            ORDER BY priority ASC, name ASC
            "#,
        )
        .fetch_all(db)
        .await?;

        let rules: Vec<FileTypeRule> = rows.into_iter().map(rule_from_row).collect();
        Self::from_rules(rules)
    }

    pub fn from_rules(rules: Vec<FileTypeRule>) -> AppResult<Self> {
        let mut exact = HashMap::new();
        let mut directories = Vec::new();
        let mut extensions = HashMap::new();
        let mut fallback = None;

        for rule in rules {
            let compiled = CompiledRule::new(rule)?;
            match compiled.rule.rule_kind.as_str() {
                "exact" => {
                    let key =
                        compiled.rule.exact_path.clone().ok_or_else(|| {
                            AppError::bad_request("exact rule missing exact_path")
                        })?;
                    exact.insert(normalize_depot_path(&key)?, compiled);
                }
                "directory" => {
                    directories.push(compiled);
                }
                "extension" => {
                    let ext = compiled
                        .rule
                        .extension
                        .clone()
                        .ok_or_else(|| AppError::bad_request("extension rule missing extension"))?
                        .to_ascii_lowercase();
                    extensions.insert(ext, compiled);
                }
                "fallback" => fallback = Some(compiled),
                other => {
                    return Err(AppError::bad_request(format!(
                        "unsupported rule kind: {other}"
                    )))
                }
            }
        }

        directories.sort_by_key(|rule| rule.rule.priority);
        let fallback =
            fallback.ok_or_else(|| AppError::configuration("missing fallback file type rule"))?;
        Ok(Self {
            exact,
            directories,
            extensions,
            fallback,
        })
    }

    pub fn match_path(&self, depot_path: &str) -> AppResult<FileTypeRule> {
        let normalized = normalize_depot_path(depot_path)?;
        if let Some(rule) = self.exact.get(&normalized) {
            return Ok(rule.rule.clone());
        }

        for rule in &self.directories {
            if rule.matches(&normalized) {
                return Ok(rule.rule.clone());
            }
        }

        if let Some(ext) = extension_of(&normalized) {
            if let Some(rule) = self.extensions.get(&ext) {
                return Ok(rule.rule.clone());
            }
        }

        Ok(self.fallback.rule.clone())
    }
}

impl CompiledRule {
    fn new(rule: FileTypeRule) -> AppResult<Self> {
        let glob = match &rule.glob {
            Some(value) => Some(
                Glob::new(value)
                    .map_err(|err| {
                        AppError::bad_request(format!("invalid glob in {}: {err}", rule.name))
                    })?
                    .compile_matcher(),
            ),
            None => None,
        };
        let regex = match &rule.regex {
            Some(value) => Some(Regex::new(value).map_err(|err| {
                AppError::bad_request(format!("invalid regex in {}: {err}", rule.name))
            })?),
            None => None,
        };
        Ok(Self { rule, glob, regex })
    }

    fn matches(&self, normalized_path: &str) -> bool {
        if let Some(prefix) = &self.rule.directory_prefix {
            if normalized_path.starts_with(prefix) {
                return true;
            }
        }
        if let Some(glob) = &self.glob {
            if glob.is_match(normalized_path) {
                return true;
            }
        }
        if let Some(regex) = &self.regex {
            if regex.is_match(normalized_path) {
                return true;
            }
        }
        false
    }
}

pub async fn list_filetypes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<FileTypeRule>>> {
    let _user = state.require_user(&headers)?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, priority, rule_kind, exact_path, directory_prefix, extension, glob, regex,
               asset_class, is_binary, lock_required, large_file, generated, enabled
        FROM file_type_rules
        ORDER BY priority ASC, name ASC
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(rule_from_row).collect()))
}

#[derive(Debug, Deserialize)]
pub struct MatchFileTypeRequest {
    pub path: String,
}

pub async fn match_filetype(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<MatchFileTypeRequest>,
) -> AppResult<Json<FileTypeRule>> {
    let _user = state.require_user(&headers)?;
    let matcher = state.filetypes.read().await;
    Ok(Json(matcher.match_path(&req.path)?))
}

fn rule_from_row(row: sqlx::postgres::PgRow) -> FileTypeRule {
    FileTypeRule {
        id: row.get("id"),
        name: row.get("name"),
        priority: row.get("priority"),
        rule_kind: row.get("rule_kind"),
        exact_path: row.get("exact_path"),
        directory_prefix: row.get("directory_prefix"),
        extension: row.get("extension"),
        glob: row.get("glob"),
        regex: row.get("regex"),
        asset_class: row.get("asset_class"),
        is_binary: row.get("is_binary"),
        lock_required: row.get("lock_required"),
        large_file: row.get("large_file"),
        generated: row.get("generated"),
        enabled: row.get("enabled"),
    }
}

fn extension_of(path: &str) -> Option<String> {
    let file_name = path.rsplit('/').next()?;
    let index = file_name.rfind('.')?;
    Some(file_name[index..].to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{FileTypeMatcher, FileTypeRule};
    use uuid::Uuid;

    fn rule(
        kind: &str,
        extension: Option<&str>,
        directory: Option<&str>,
        lock: bool,
        asset_class: &str,
    ) -> FileTypeRule {
        FileTypeRule {
            id: Uuid::new_v4(),
            name: format!("{kind}-{asset_class}"),
            priority: 10,
            rule_kind: kind.to_string(),
            exact_path: None,
            directory_prefix: directory.map(str::to_string),
            extension: extension.map(str::to_string),
            glob: None,
            regex: None,
            asset_class: asset_class.to_string(),
            is_binary: true,
            lock_required: lock,
            large_file: false,
            generated: false,
            enabled: true,
        }
    }

    #[test]
    fn extension_rules_require_locks() {
        let matcher = FileTypeMatcher::from_rules(vec![
            rule("extension", Some(".umap"), None, true, "unreal_map"),
            rule("fallback", None, None, false, "generic"),
        ])
        .unwrap();
        let matched = matcher.match_path("Content/Maps/Level.umap").unwrap();
        assert!(matched.lock_required);
    }

    #[test]
    fn directory_precedence_beats_extension() {
        let matcher = FileTypeMatcher::from_rules(vec![
            rule("directory", None, Some("Library/"), false, "generated"),
            rule("extension", Some(".uasset"), None, true, "unreal"),
            rule("fallback", None, None, false, "generic"),
        ])
        .unwrap();
        let matched = matcher.match_path("Library/Cache/File.uasset").unwrap();
        assert_eq!(matched.asset_class, "generated");
    }

    #[test]
    fn exact_precedence_beats_directory_and_extension() {
        let mut exact = rule("exact", None, None, false, "project_descriptor");
        exact.exact_path = Some("Project/Config.asset".to_string());
        let matcher = FileTypeMatcher::from_rules(vec![
            exact,
            rule(
                "directory",
                None,
                Some("Project/"),
                false,
                "project_directory",
            ),
            rule("extension", Some(".asset"), None, true, "unity_asset"),
            rule("fallback", None, None, false, "generic"),
        ])
        .unwrap();
        let matched = matcher.match_path("Project/Config.asset").unwrap();
        assert_eq!(matched.asset_class, "project_descriptor");
        assert!(!matched.lock_required);
    }

    #[test]
    fn invalid_regex_rules_are_rejected_at_startup() {
        let mut regex_rule = rule("directory", None, Some("Assets/"), false, "assets");
        regex_rule.regex = Some("(".to_string());
        let result = FileTypeMatcher::from_rules(vec![
            regex_rule,
            rule("fallback", None, None, false, "generic"),
        ]);
        assert!(result.is_err());
    }
}
