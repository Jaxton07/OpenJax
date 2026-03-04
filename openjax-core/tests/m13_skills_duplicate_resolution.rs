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
fn duplicate_skills_follow_priority_workspace_then_user_and_first_root_wins() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cwd = tmp.path().join("workspace");
    let home = tmp.path().join("home");

    write_skill(
        &cwd.join(".openjax/skills"),
        "main",
        "Build Skill",
        "workspace-openjax",
    );
    write_skill(
        &cwd.join(".claude/skills"),
        "alt",
        "build skill",
        "workspace-claude",
    );
    write_skill(
        &home.join(".openjax/skills"),
        "user",
        "BUILD SKILL",
        "user-openjax",
    );

    let registry = SkillRegistry::load_from_locations(&cwd, Some(home.as_path()));
    assert_eq!(registry.entries.len(), 1);
    assert_eq!(
        registry.entries[0].manifest.description,
        "workspace-openjax"
    );
}
