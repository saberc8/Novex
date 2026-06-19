use serde::{Deserialize, Serialize};

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
