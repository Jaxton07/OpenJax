use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum PatchOperation {
    AddFile { path: String, lines: Vec<String> },
    DeleteFile { path: String },
    UpdateFile { path: String, move_to: Option<String>, hunks: Vec<PatchHunk> },
    MoveFile { from: String, to: String },
    RenameFile { from: String, to: String },
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
    Create { path: PathBuf, content: String },
    Update { path: PathBuf, content: String },
    Delete { path: PathBuf },
    Move { from: PathBuf, to: PathBuf },
}

impl PlannedAction {
    pub fn path(&self) -> &Path {
        match self {
            Self::Create { path, .. }
            | Self::Update { path, .. }
            | Self::Delete { path }
            | Self::Move { to: path, .. } => {
                path.as_path()
            }
        }
    }

    pub fn summary(&self, cwd: &Path) -> String {
        match self {
            Self::Create { path, .. } => format!("ADD {}", super::matcher::display_rel_path(cwd, path)),
            Self::Update { path, .. } => format!("UPDATE {}", super::matcher::display_rel_path(cwd, path)),
            Self::Delete { path } => format!("DELETE {}", super::matcher::display_rel_path(cwd, path)),
            Self::Move { from, to } => {
                format!(
                    "MOVE {} -> {}",
                    super::matcher::display_rel_path(cwd, from),
                    super::matcher::display_rel_path(cwd, to)
                )
            }
        }
    }
}
