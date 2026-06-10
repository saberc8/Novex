use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillResourceKind {
    SkillMd,
    Reference,
    Script,
    Asset,
    Metadata,
    Ignored,
}

impl SkillResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkillMd => "skill_md",
            Self::Reference => "reference",
            Self::Script => "script",
            Self::Asset => "asset",
            Self::Metadata => "metadata",
            Self::Ignored => "ignored",
        }
    }

    pub fn is_text_resource(self, mime_type: &str) -> bool {
        matches!(
            self,
            Self::SkillMd | Self::Reference | Self::Script | Self::Metadata
        ) || mime_type.starts_with("text/")
            || matches!(
                mime_type,
                "application/json" | "application/yaml" | "application/x-yaml"
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillPackageError {
    EmptyPath,
    InvalidPath,
    PathTraversal,
    MissingSkillManifest,
    MultipleSkillManifests,
}

impl fmt::Display for SkillPackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => f.write_str("skill package path is empty"),
            Self::InvalidPath => f.write_str("skill package path is invalid"),
            Self::PathTraversal => f.write_str("skill package path contains traversal"),
            Self::MissingSkillManifest => f.write_str("skill package is missing SKILL.md"),
            Self::MultipleSkillManifests => {
                f.write_str("skill package contains multiple SKILL.md files")
            }
        }
    }
}

impl Error for SkillPackageError {}

pub trait SkillPackagePath {
    fn relative_path(&self) -> &str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillPackageFile<'a> {
    pub relative_path: &'a str,
}

impl SkillPackagePath for SkillPackageFile<'_> {
    fn relative_path(&self) -> &str {
        self.relative_path
    }
}

pub fn normalize_skill_package_path(path: &str) -> Result<String, SkillPackageError> {
    let path = path.replace('\\', "/");
    let path = path.trim().trim_start_matches("./").trim_matches('/');
    if path.is_empty() {
        return Err(SkillPackageError::EmptyPath);
    }
    if path.starts_with('/') || path.contains('\0') {
        return Err(SkillPackageError::InvalidPath);
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        let part = part.trim();
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(SkillPackageError::PathTraversal);
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(SkillPackageError::EmptyPath);
    }
    Ok(parts.join("/"))
}

pub fn normalize_skill_package_path_or_empty(path: &str) -> Result<String, SkillPackageError> {
    let path = path.trim().trim_matches('/');
    if path.is_empty() {
        Ok(String::new())
    } else {
        normalize_skill_package_path(path)
    }
}

pub fn selected_skill_md_index<P: SkillPackagePath>(
    files: &[P],
) -> Result<usize, SkillPackageError> {
    if let Some(index) = files
        .iter()
        .position(|file| file.relative_path().eq_ignore_ascii_case("SKILL.md"))
    {
        return Ok(index);
    }

    let matches = files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.relative_path().ends_with("/SKILL.md"))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(SkillPackageError::MissingSkillManifest),
        _ => Err(SkillPackageError::MultipleSkillManifests),
    }
}

pub fn skill_root_from_skill_md_path(skill_md_path: &str) -> String {
    skill_md_path
        .strip_suffix("/SKILL.md")
        .unwrap_or("")
        .trim_matches('/')
        .to_owned()
}

pub fn strip_skill_root(skill_root: &str, path: &str) -> Option<String> {
    if skill_root.is_empty() {
        return Some(path.to_owned());
    }
    path.strip_prefix(skill_root)
        .and_then(|value| value.strip_prefix('/'))
        .map(ToOwned::to_owned)
}

pub fn skill_resource_kind(path: &str) -> SkillResourceKind {
    let lower = path.to_ascii_lowercase();
    if lower == "skill.md" {
        SkillResourceKind::SkillMd
    } else if lower.starts_with("references/") {
        SkillResourceKind::Reference
    } else if lower.starts_with("scripts/") {
        SkillResourceKind::Script
    } else if lower.starts_with("assets/") {
        SkillResourceKind::Asset
    } else if lower == "agents/openai.yaml" || lower == "agents/openai.yml" {
        SkillResourceKind::Metadata
    } else {
        SkillResourceKind::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_skill_package_paths_and_rejects_traversal() {
        assert_eq!(
            normalize_skill_package_path(r".\writer\references\guide.md").unwrap(),
            "writer/references/guide.md"
        );
        assert_eq!(
            normalize_skill_package_path_or_empty(" /writer/ ").unwrap(),
            "writer"
        );
        assert_eq!(normalize_skill_package_path_or_empty(" / ").unwrap(), "");

        assert!(normalize_skill_package_path("../SKILL.md").is_err());
        assert!(normalize_skill_package_path("writer/../SKILL.md").is_err());
        assert!(normalize_skill_package_path("writer/\0/SKILL.md").is_err());
    }

    #[test]
    fn identifies_exactly_one_skill_manifest_and_root() {
        let files = [
            SkillPackageFile {
                relative_path: "writer/references/style.md",
            },
            SkillPackageFile {
                relative_path: "writer/SKILL.md",
            },
        ];

        let index = selected_skill_md_index(&files).unwrap();
        assert_eq!(index, 1);
        assert_eq!(
            skill_root_from_skill_md_path(files[index].relative_path),
            "writer"
        );
        assert_eq!(
            strip_skill_root("writer", "writer/references/style.md"),
            Some("references/style.md".to_owned())
        );

        let multiple = [
            SkillPackageFile {
                relative_path: "first/SKILL.md",
            },
            SkillPackageFile {
                relative_path: "second/SKILL.md",
            },
        ];
        assert!(matches!(
            selected_skill_md_index(&multiple),
            Err(SkillPackageError::MultipleSkillManifests)
        ));
    }

    #[test]
    fn classifies_codex_skill_resource_layout() {
        assert_eq!(skill_resource_kind("SKILL.md"), SkillResourceKind::SkillMd);
        assert_eq!(
            skill_resource_kind("references/source.md"),
            SkillResourceKind::Reference
        );
        assert_eq!(
            skill_resource_kind("scripts/setup.sh"),
            SkillResourceKind::Script
        );
        assert_eq!(
            skill_resource_kind("assets/icon.png"),
            SkillResourceKind::Asset
        );
        assert_eq!(
            skill_resource_kind("agents/openai.yaml"),
            SkillResourceKind::Metadata
        );
        assert_eq!(skill_resource_kind("README.md"), SkillResourceKind::Ignored);

        assert_eq!(SkillResourceKind::Script.as_str(), "script");
        assert!(SkillResourceKind::Reference.is_text_resource("application/pdf"));
        assert!(SkillResourceKind::Asset.is_text_resource("text/plain"));
        assert!(!SkillResourceKind::Asset.is_text_resource("image/png"));
    }
}
