use super::types::CommandClass;

pub fn classify_command(command: &str) -> CommandClass {
    let normalized = command.trim_start().to_ascii_lowercase();
    if normalized.starts_with("ps ")
        || normalized.starts_with("top ")
        || normalized.starts_with("pgrep ")
    {
        return CommandClass::ProcessObserve;
    }
    if normalized.contains("curl ") || normalized.contains("wget ") || normalized.contains("ssh ") {
        return CommandClass::NetworkHeavy;
    }
    if normalized.contains('>')
        || normalized.contains(" tee ")
        || normalized.contains(" rm ")
        || normalized.starts_with("rm ")
        || normalized.contains(" mv ")
        || normalized.contains(" cp ")
    {
        return CommandClass::WriteHeavy;
    }
    CommandClass::General
}

#[cfg(test)]
mod tests {
    use super::{CommandClass, classify_command};

    #[test]
    fn identifies_process_observe_commands() {
        assert_eq!(
            classify_command("ps aux --sort=-%cpu | head -5"),
            CommandClass::ProcessObserve
        );
        assert_eq!(
            classify_command("top -l 1 -n 5 -o cpu"),
            CommandClass::ProcessObserve
        );
        assert_eq!(
            classify_command("pgrep -l zsh"),
            CommandClass::ProcessObserve
        );
    }
}
