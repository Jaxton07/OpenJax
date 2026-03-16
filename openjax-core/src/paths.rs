use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenJaxPaths {
    pub root_dir: PathBuf,
    pub config_file: PathBuf,
    pub logs_dir: PathBuf,
    pub history_file: PathBuf,
    pub skills_dir: PathBuf,
    pub database_dir: PathBuf,
}

impl OpenJaxPaths {
    pub fn detect() -> Option<Self> {
        let home_dir = dirs::home_dir()?;
        Some(Self::from_home_dir(home_dir))
    }

    pub fn from_home_dir(home_dir: impl AsRef<Path>) -> Self {
        let root_dir = home_dir.as_ref().join(".openjax");
        Self {
            config_file: root_dir.join("config.toml"),
            logs_dir: root_dir.join("logs"),
            history_file: root_dir.join("history.txt"),
            skills_dir: root_dir.join("skills"),
            database_dir: root_dir.join("database"),
            root_dir,
        }
    }

    pub fn ensure_runtime_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.root_dir)?;
        fs::create_dir_all(&self.logs_dir)?;
        fs::create_dir_all(&self.skills_dir)?;
        fs::create_dir_all(&self.database_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::OpenJaxPaths;

    #[test]
    fn builds_expected_layout_from_home_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = OpenJaxPaths::from_home_dir(tmp.path());

        assert_eq!(paths.root_dir, tmp.path().join(".openjax"));
        assert_eq!(paths.config_file, tmp.path().join(".openjax/config.toml"));
        assert_eq!(paths.logs_dir, tmp.path().join(".openjax/logs"));
        assert_eq!(paths.history_file, tmp.path().join(".openjax/history.txt"));
        assert_eq!(paths.skills_dir, tmp.path().join(".openjax/skills"));
        assert_eq!(paths.database_dir, tmp.path().join(".openjax/database"));
    }

    #[test]
    fn ensure_runtime_dirs_creates_root_logs_and_skills() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = OpenJaxPaths::from_home_dir(tmp.path());

        paths.ensure_runtime_dirs().expect("ensure runtime dirs");

        assert!(paths.root_dir.is_dir());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.skills_dir.is_dir());
        assert!(paths.database_dir.is_dir());
    }
}
