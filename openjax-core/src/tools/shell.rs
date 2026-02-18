use anyhow::Result;
use std::path::PathBuf;
use which::which;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    PowerShell,
}

impl Default for ShellType {
    fn default() -> Self {
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL").unwrap_or_default();
            if shell.contains("bash") {
                Self::Bash
            } else if shell.contains("zsh") {
                Self::Zsh
            } else {
                Self::Zsh
            }
        }
        #[cfg(windows)]
        Self::PowerShell
    }
}

impl ShellType {
    pub fn executable_name(&self) -> &str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::PowerShell => "pwsh",
        }
    }

    pub fn login_flag(&self) -> &str {
        match self {
            Self::Bash => "--login",
            Self::Zsh => "-l",
            Self::PowerShell => "-Login",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Shell {
    pub shell_type: ShellType,
    pub shell_path: PathBuf,
}

impl Shell {
    pub fn new(shell_type: ShellType) -> Result<Self> {
        let executable = shell_type.executable_name();
        let shell_path = which(executable)
            .map_err(|_| anyhow::anyhow!("{} not found", executable))?;

        Ok(Self {
            shell_type,
            shell_path,
        })
    }

    pub fn derive_exec_args(&self, command: &str, use_login_shell: Option<bool>) -> Vec<String> {
        let login_flag = use_login_shell.unwrap_or(true);
        let flag = self.shell_type.login_flag();
        let args = if login_flag {
            vec![flag.to_string(), "-c".to_string(), command.to_string()]
        } else {
            vec!["-c".to_string(), command.to_string()]
        };

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_executable_name() {
        assert_eq!(ShellType::Bash.executable_name(), "bash");
        assert_eq!(ShellType::Zsh.executable_name(), "zsh");
        assert_eq!(ShellType::PowerShell.executable_name(), "pwsh");
    }

    #[test]
    fn test_shell_type_login_flag() {
        assert_eq!(ShellType::Bash.login_flag(), "--login");
        assert_eq!(ShellType::Zsh.login_flag(), "-l");
        assert_eq!(ShellType::PowerShell.login_flag(), "-Login");
    }

    #[test]
    fn test_shell_derive_exec_args_with_login() {
        let shell = Shell::new(ShellType::Bash).unwrap();
        let args = shell.derive_exec_args("echo hello", Some(true));
        assert_eq!(args, vec!["--login", "-c", "echo hello"]);
    }

    #[test]
    fn test_shell_derive_exec_args_without_login() {
        let shell = Shell::new(ShellType::Zsh).unwrap();
        let args = shell.derive_exec_args("echo hello", Some(false));
        assert_eq!(args, vec!["-c", "echo hello"]);
    }
}