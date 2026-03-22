use openjax_core::skills::{SkillRegistry, build_skills_context};
use std::fs;

#[test]
fn skills_context_contains_non_shell_trigger_rule() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let skills_root = tmp.path().join("home/.openjax/skills");
    let skill_dir = skills_root.join("local-commit");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: local-commit\ndescription: commit local changes\n---\n\
         trigger `/local-commit` should map to workflow steps",
    )
    .expect("write skill");

    let registry = SkillRegistry::load_from_locations(&skills_root);
    let selected = registry.select_for_input("请 commit 本地修改", 3);
    let context = build_skills_context(&selected, 6000);

    assert!(context.contains("not a shell executable"));
    assert!(context.contains("Do NOT call shell with only a trigger string"));
}
