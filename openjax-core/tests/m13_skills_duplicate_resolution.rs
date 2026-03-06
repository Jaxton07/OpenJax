use openjax_core::skills::SkillRegistry;
use std::fs;
use std::path::Path;

fn write_skill(root: &Path, package_name: &str, skill_name: &str, description: &str) {
    let dir = root.join(package_name);
    fs::create_dir_all(&dir).expect("create skill dir");
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {skill_name}\ndescription: {description}\n---\nbody"),
    )
    .expect("write skill file");
}

#[test]
fn duplicate_skills_follow_first_directory_wins_within_user_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let skills_root = tmp.path().join("home/.openjax/skills");

    write_skill(&skills_root, "main", "Build Skill", "user-openjax");
    write_skill(&skills_root, "alt", "build skill", "user-alt");

    let registry = SkillRegistry::load_from_locations(&skills_root);
    assert_eq!(registry.entries.len(), 1);
    assert_eq!(registry.entries[0].manifest.description, "user-alt");
}
