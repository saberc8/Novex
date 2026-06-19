use novex_skill::{
    normalize_skill_package_path, normalize_skill_package_path_or_empty, selected_skill_md_index,
    skill_root_from_skill_md_path, strip_skill_root, SkillPackageError, SkillPackageFile,
};

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
