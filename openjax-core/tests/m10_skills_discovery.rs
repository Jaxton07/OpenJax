use openjax_core::skills::SkillRegistry;
use std::fs;
use std::path::PathBuf;

fn write_skill(root: &PathBuf, dir_name: &str, name: &str, description: &str) {
    let dir = root.join(dir_name);
    fs::create_dir_all(&dir).expect("create skill dir");
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\nUse this skill for {name}."),
    )
    .expect("write skill file");
}

#[test]
fn discovers_skills_across_openjax_claude_openclaw_roots() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cwd = tmp.path().join("workspace");
    let home = tmp.path().join("home");

    write_skill(
        &cwd.join(".openjax/skills"),
        "rust-debug",
        "Rust Debug",
        "debug rust compiler issues",
    );
    write_skill(
        &cwd.join(".claude/skills"),
        "python-tests",
        "Python Tests",
        "run python unit tests",
    );
    write_skill(
        &home.join(".openclaw/skills"),
        "release-check",
        "Release Check",
        "release checklist",
    );

    let registry = SkillRegistry::load_from_locations(&cwd, Some(home.as_path()));
    assert_eq!(registry.entries.len(), 3);
    assert!(registry.discovered_count >= 3);
}
