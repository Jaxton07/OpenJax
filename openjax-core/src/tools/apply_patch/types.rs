use std::path::{Path, PathBuf};

/// The line range (1-indexed, inclusive) in the result file where a patch hunk landed.
#[derive(Debug, Clone)]
pub struct HunkResultRange {
    pub start: usize,
    pub end: usize,
}

/// Warnings produced during fuzzy hunk matching.
#[derive(Debug, Clone)]
pub enum HunkWarning {
    /// Hunk matched using a fuzzy strategy at the given level (1=trim_end, 2=trim, 3=unicode).
    FuzzyMatch { hunk_index: usize, level: usize },
    /// Multiple match locations found; patch applied to the first occurrence.
    AmbiguousMatch { hunk_index: usize },
}

#[derive(Debug, Clone)]
pub enum PatchOperation {
    AddFile {
        path: String,
        lines: Vec<String>,
    },
    DeleteFile {
        path: String,
    },
    UpdateFile {
        path: String,
        move_to: Option<String>,
        hunks: Vec<PatchHunk>,
    },
    MoveFile {
        from: String,
        to: String,
    },
    RenameFile {
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub context: Option<String>,
    pub lines: Vec<PatchHunkLine>,
}

#[derive(Debug, Clone)]
pub struct PatchHunkLine {
    pub kind: PatchLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLineKind {
    Context,
    Remove,
    Add,
}

#[derive(Debug, Clone)]
pub enum PlannedAction {
    Create {
        path: PathBuf,
        content: String,
    },
    Update {
        path: PathBuf,
        content: String,
        changed_ranges: Vec<HunkResultRange>,
        warnings: Vec<HunkWarning>,
    },
    Delete {
        path: PathBuf,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
    },
}

impl PlannedAction {
    pub fn path(&self) -> &Path {
        match self {
            Self::Create { path, .. }
            | Self::Update { path, .. }
            | Self::Delete { path }
            | Self::Move { to: path, .. } => path.as_path(),
        }
    }

    pub fn summary(&self, cwd: &Path) -> String {
        match self {
            Self::Create { path, .. } => {
                format!("ADD {}", super::matcher::display_rel_path(cwd, path))
            }
            Self::Update { path, .. } => {
                format!("UPDATE {}", super::matcher::display_rel_path(cwd, path))
            }
            Self::Delete { path } => {
                format!("DELETE {}", super::matcher::display_rel_path(cwd, path))
            }
            Self::Move { from, to } => {
                format!(
                    "MOVE {} -> {}",
                    super::matcher::display_rel_path(cwd, from),
                    super::matcher::display_rel_path(cwd, to),
                )
            }
        }
    }
}
