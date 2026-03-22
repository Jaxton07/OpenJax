//! Aggregated integration suite for skill discovery, selection, and runtime guards.

#[path = "skills/m10_skills_discovery.rs"]
mod skills_discovery_m10;
#[path = "skills/m11_skills_prompt_injection.rs"]
mod skills_prompt_injection_m11;
#[path = "skills/m12_skills_config_toggle.rs"]
mod skills_config_toggle_m12;
#[path = "skills/m13_skills_duplicate_resolution.rs"]
mod skills_duplicate_resolution_m13;
#[path = "skills/m18_skills_no_shell_trigger_misfire.rs"]
mod skills_no_shell_trigger_m18;
#[path = "skills/m19_skills_lightweight_commit_flow.rs"]
mod skills_lightweight_commit_flow_m19;
#[path = "skills/m20_skills_shell_trigger_guard.rs"]
mod skills_shell_trigger_guard_m20;
