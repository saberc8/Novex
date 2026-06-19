use std::{error::Error, fmt};

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
