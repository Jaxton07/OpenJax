use openjax_core::skills::SkillRegistry;
use std::fs;
use std::path::Path;

fn write_skill(root: &Path, dir_name: &str, name: &str, description: &str) {
    let dir = root.join(dir_name);
    fs::create_dir_all(&dir).expect("create skill dir");
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\nUse this skill for {name}."),
    )
    .expect("write skill file");
}

#[test]
fn discovers_skills_only_from_openjax_user_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let skills_root = tmp.path().join("home/.openjax/skills");

    write_skill(
        &skills_root,
        "rust-debug",
        "Rust Debug",
        "debug rust compiler issues",
    );

    let registry = SkillRegistry::load_from_locations(&skills_root);
    assert_eq!(registry.entries.len(), 1);
    assert_eq!(registry.discovered_count, 1);
}
