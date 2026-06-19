use std::fs;
use std::path::Path;

use novex_skill::{
    normalize_skill_package_path, selected_skill_md_index, skill_resource_kind,
    skill_root_from_skill_md_path, strip_skill_root, SkillPackageFile, SkillPackagePath,
    SkillResourceKind,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_skill_modules() {
    let lib = crate_file("src/lib.rs");

    for module in ["path", "resource"] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub enum SkillResourceKind",
        "pub enum SkillPackageError",
        "pub trait SkillPackagePath",
        "pub struct SkillPackageFile",
        "pub fn normalize_skill_package_path",
        "pub fn selected_skill_md_index",
        "pub fn skill_resource_kind",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn skill_domain_modules_exist() {
    for module in ["src/path.rs", "src/resource.rs"] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_skill_contracts() {
    assert_eq!(
        normalize_skill_package_path(r".\writer\references\guide.md").unwrap(),
        "writer/references/guide.md"
    );
    let files = [
        SkillPackageFile {
            relative_path: "writer/references/style.md",
        },
        SkillPackageFile {
            relative_path: "writer/SKILL.md",
        },
    ];
    let index = selected_skill_md_index(&files).unwrap();
    assert_eq!(files[index].relative_path(), "writer/SKILL.md");
    assert_eq!(skill_root_from_skill_md_path("writer/SKILL.md"), "writer");
    assert_eq!(
        strip_skill_root("writer", "writer/references/style.md"),
        Some("references/style.md".to_owned())
    );
    assert_eq!(skill_resource_kind("SKILL.md"), SkillResourceKind::SkillMd);
    assert!(SkillResourceKind::Script.is_text_resource("application/json"));
}
