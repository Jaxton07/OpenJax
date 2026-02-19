pub mod applier;
pub mod heredoc;
pub mod matcher;
pub mod parser;
pub mod planner;
pub mod tool;
pub mod types;

pub use applier::{apply_hunks_to_content, apply_patch_actions};
pub use heredoc::{extract_heredoc, normalize_patch_arg};
pub use matcher::{display_rel_path, find_subsequence, seek_sequence, split_lines_preserve_end};
pub use parser::parse_apply_patch;
pub use planner::plan_patch_actions;
pub use tool::apply_patch_tool;
pub use types::{PatchHunk, PatchHunkLine, PatchLineKind, PatchOperation, PlannedAction};
