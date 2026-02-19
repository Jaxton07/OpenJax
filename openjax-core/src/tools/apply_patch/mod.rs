pub mod types;
pub mod parser;
pub mod heredoc;
pub mod matcher;
pub mod applier;
pub mod planner;
pub mod tool;

pub use types::{PatchOperation, PatchHunk, PatchHunkLine, PatchLineKind, PlannedAction};
pub use parser::parse_apply_patch;
pub use heredoc::{normalize_patch_arg, extract_heredoc};
pub use matcher::{split_lines_preserve_end, find_subsequence, seek_sequence, display_rel_path};
pub use applier::{apply_patch_actions, apply_hunks_to_content};
pub use planner::plan_patch_actions;
pub use tool::apply_patch_tool;
