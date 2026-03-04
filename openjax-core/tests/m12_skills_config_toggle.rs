use std::collections::HashMap;

use openjax_core::skills::SkillRuntimeConfig;

#[test]
fn runtime_config_defaults_and_options_are_applied() {
    let cfg = SkillRuntimeConfig::from_options(None, None, None);
    assert!(cfg.enabled);
    assert_eq!(cfg.max_selected, 3);
    assert_eq!(cfg.max_prompt_chars, 6000);

    let cfg = SkillRuntimeConfig::from_options(Some(false), Some(8), Some(9000));
    assert!(!cfg.enabled);
    assert_eq!(cfg.max_selected, 8);
    assert_eq!(cfg.max_prompt_chars, 9000);
}

#[test]
fn env_values_override_config_options() {
    let cfg = SkillRuntimeConfig::from_options(Some(true), Some(3), Some(6000));
    let mut env = HashMap::new();
    env.insert("OPENJAX_SKILLS_ENABLED".to_string(), "false".to_string());
    env.insert("OPENJAX_SKILLS_MAX_SELECTED".to_string(), "9".to_string());
    env.insert(
        "OPENJAX_SKILLS_MAX_PROMPT_CHARS".to_string(),
        "12000".to_string(),
    );

    let resolved = cfg.apply_env_with_lookup(|key| env.get(key).cloned());
    assert!(!resolved.enabled);
    assert_eq!(resolved.max_selected, 9);
    assert_eq!(resolved.max_prompt_chars, 12000);
}
