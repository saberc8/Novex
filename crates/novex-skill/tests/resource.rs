use novex_skill::{skill_resource_kind, SkillResourceKind};

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
