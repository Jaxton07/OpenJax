use openjax_core::skills::{SkillRegistry, build_skills_context};
use std::fs;

#[test]
fn selected_skills_render_prompt_context_with_name_description_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cwd = tmp.path().join("workspace");
    let skill_dir = cwd.join(".openjax/skills/rust-debug");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: Rust Debug\ndescription: Fix rust compile failures\n---\nUse cargo check then cargo test.",
    )
    .expect("write skill");

    let registry = SkillRegistry::load_from_locations(&cwd, None);
    let selected = registry.select_for_input("please debug rust compile error", 3);
    assert_eq!(selected.len(), 1);

    let context = build_skills_context(&selected, 6000);
    assert!(context.contains("name: Rust Debug"));
    assert!(context.contains("description: Fix rust compile failures"));
    assert!(context.contains(".openjax/skills/rust-debug"));
    assert!(context.contains("Use cargo check then cargo test"));
}
