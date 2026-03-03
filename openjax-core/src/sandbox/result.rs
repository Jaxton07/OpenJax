use super::types::SandboxResultClass;

pub fn classify_shell_result(exit_code: i32, stdout: &str, stderr: &str) -> SandboxResultClass {
    let stderr_trimmed = stderr.trim();
    let stdout_trimmed = stdout.trim();
    if exit_code == 0 {
        if stdout_trimmed.is_empty() && looks_like_fatal_stderr(stderr_trimmed) {
            return SandboxResultClass::Failure;
        }
        return SandboxResultClass::Success;
    }
    if exit_code == 141 && !stdout_trimmed.is_empty() && stderr_trimmed.is_empty() {
        return SandboxResultClass::PartialSuccess;
    }
    SandboxResultClass::Failure
}

pub fn looks_like_fatal_stderr(stderr: &str) -> bool {
    if stderr.is_empty() {
        return false;
    }
    let lower = stderr.to_ascii_lowercase();
    lower.contains("operation not permitted")
        || lower.contains("permission denied")
        || lower.contains("command not found")
        || lower.contains("illegal option")
        || lower.contains("no such file or directory")
}

#[cfg(test)]
mod tests {
    use super::{classify_shell_result, looks_like_fatal_stderr};
    use crate::sandbox::types::SandboxResultClass;

    #[test]
    fn classifies_zero_exit_with_fatal_stderr_as_failure() {
        assert_eq!(
            classify_shell_result(0, "", "/bin/sh: /bin/ps: Operation not permitted"),
            SandboxResultClass::Failure
        );
    }

    #[test]
    fn classifies_sigpipe_with_output_as_partial_success() {
        assert_eq!(
            classify_shell_result(141, "some output", ""),
            SandboxResultClass::PartialSuccess
        );
    }

    #[test]
    fn detects_fatal_stderr() {
        assert!(looks_like_fatal_stderr("Permission denied"));
    }
}
