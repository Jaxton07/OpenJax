use openjax_core::skills::{SkillRegistry, build_skills_context};
use std::fs;

#[test]
fn skills_context_includes_lightweight_commit_best_practice() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let skills_root = tmp.path().join("home/.openjax/skills");
    let skill_dir = skills_root.join("local-commit");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: local-commit\ndescription: commit local changes\n---\n\
         1) git status --short\n2) git diff --stat\n3) add and commit",
    )
    .expect("write skill");

    let registry = SkillRegistry::load_from_locations(&skills_root);
    let selected = registry.select_for_input("你commit一下本地的修改", 3);
    let context = build_skills_context(&selected, 6000);

    assert!(context.contains("git status --short"));
    assert!(context.contains("git diff --stat"));
    assert!(context.contains("expand diff only if needed"));
}
