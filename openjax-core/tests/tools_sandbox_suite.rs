//! Aggregated integration suite for tool execution, sandbox policy, and file mutation flows.

#[path = "tools_sandbox/m4_apply_patch.rs"]
mod apply_patch_m4;
#[path = "tools_sandbox/m5_edit_file_range.rs"]
mod edit_file_range_m5;
#[path = "tools_sandbox/m3_sandbox.rs"]
mod sandbox_m3;
#[path = "tools_sandbox/m9_system_tools.rs"]
mod system_tools_m9;
#[path = "tools_sandbox/m10_write_file.rs"]
mod write_file_m10;
#[path = "tools_sandbox/m11_glob_files.rs"]
mod glob_files_m11;
