use std::process::Command;

/// Windows `CREATE_NO_WINDOW`. Background helper processes (git reads,
/// version probes, taskkill) must never flash a console window while the
/// desktop app reads project information.
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Mark a child process as a background helper so it does not open a console
/// window on Windows. No-op on other platforms, where child processes never
/// surface a console.
pub(crate) fn suppress_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

#[cfg(test)]
mod tests {
    use super::suppress_console_window;
    use std::process::Command;

    #[test]
    fn accepts_any_command_without_panicking() {
        let mut command = Command::new("git");
        suppress_console_window(&mut command);
    }
}
