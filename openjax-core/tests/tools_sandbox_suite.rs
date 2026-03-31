//! Aggregated integration suite for tool execution, sandbox policy, and file mutation flows.

#[path = "tools_sandbox/m5_edit.rs"]
mod edit_m5;
#[path = "tools_sandbox/m11_glob_files.rs"]
mod glob_files_m11;
#[path = "tools_sandbox/m3_sandbox.rs"]
mod sandbox_m3;
#[path = "tools_sandbox/m12_string_args_compat.rs"]
mod string_args_compat_m12;
#[path = "tools_sandbox/m9_system_tools.rs"]
mod system_tools_m9;
#[path = "tools_sandbox/m10_write_file.rs"]
mod write_file_m10;
