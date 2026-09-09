use crate::error::{Error, Result};
use crate::paths;
use crate::process::suppress_console_window;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTool {
    pub id: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTools {
    pub agents: Vec<ExternalTool>,
    pub ides: Vec<ExternalTool>,
    pub terminals: Vec<ExternalTool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsMode {
    Console,
    Gui,
}

#[derive(Debug, Clone)]
struct LaunchSpec {
    program: PathBuf,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
    windows_mode: WindowsMode,
}

#[derive(Clone)]
struct ToolBinary {
    program: PathBuf,
    extra_args: Vec<OsString>,
    cli: bool,
}

pub fn list_external_tools() -> ExternalTools {
    ExternalTools {
        agents: list_agents(),
        ides: list_ides(),
        terminals: list_terminals(),
    }
}

pub fn list_agents() -> Vec<ExternalTool> {
    discover_agents()
        .into_iter()
        .map(|(tool, _)| tool)
        .collect()
}

pub fn list_ides() -> Vec<ExternalTool> {
    discover_ides().into_iter().map(|(tool, _)| tool).collect()
}

pub fn list_terminals() -> Vec<ExternalTool> {
    discover_terminals()
        .into_iter()
        .map(|(tool, _)| tool)
        .collect()
}

pub fn open_in_file_manager(path: &str) -> Result<()> {
    let target = resolve_existing_path(path)?;
    #[cfg(windows)]
    {
        spawn_spec(LaunchSpec {
            program: windows_system_root().join("explorer.exe"),
            args: vec![target.into_os_string()],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        })
    }
    #[cfg(target_os = "macos")]
    {
        spawn_spec(LaunchSpec {
            program: PathBuf::from("open"),
            args: vec![target.into_os_string()],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = target;
        Err(Error::msg("unsupported platform"))
    }
}

pub fn open_in_ide(path: &str, ide_id: &str) -> Result<()> {
    let dir = resolve_project_dir(path)?;
    let Some((_, spec)) = discover_ides()
        .into_iter()
        .find(|(tool, _)| tool.id == ide_id)
    else {
        return Err(Error::msg(format!("IDE '{ide_id}' is not installed")));
    };
    spawn_spec(spec_for_ide(spec, &dir))
}

pub fn open_in_agent(path: &str, agent_id: &str) -> Result<()> {
    let dir = resolve_project_dir(path)?;
    let Some((_, spec)) = discover_agents()
        .into_iter()
        .find(|(tool, _)| tool.id == agent_id)
    else {
        return Err(Error::msg(format!("agent '{agent_id}' is not installed")));
    };
    spawn_spec(spec_for_agent(spec, &dir))
}

pub fn open_in_terminal(path: &str, terminal_id: Option<&str>) -> Result<()> {
    let dir = resolve_project_dir(path)?;
    let terminals = discover_terminals();
    let selected = if let Some(id) = terminal_id.filter(|id| !id.is_empty()) {
        terminals
            .into_iter()
            .find(|(tool, _)| tool.id == id)
            .ok_or_else(|| Error::msg(format!("terminal '{id}' is not installed")))?
    } else {
        terminals
            .into_iter()
            .next()
            .ok_or_else(|| Error::msg("no terminal was detected on this system"))?
    };
    spawn_spec(spec_for_terminal(selected.0.id.as_str(), selected.1, &dir))
}

pub fn resolve_project_dir(path: &str) -> Result<PathBuf> {
    let resolved = resolve_existing_path(path)?;
    if resolved.is_dir() {
        return Ok(resolved);
    }
    resolved
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty() && parent.is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            Error::msg(format!(
                "project directory does not exist: {}",
                paths::path_to_string(&resolved)
            ))
        })
}

fn resolve_existing_path(path: &str) -> Result<PathBuf> {
    let resolved = paths::canonicalize(Path::new(path))?;
    if resolved.exists() {
        Ok(resolved)
    } else {
        Err(Error::msg(format!(
            "path does not exist: {}",
            paths::path_to_string(&resolved)
        )))
    }
}

fn discover_agents() -> Vec<(ExternalTool, ToolBinary)> {
    let mut tools = Vec::new();
    push_tool(
        &mut tools,
        "cursor",
        "agent",
        "Cursor",
        find_first(&[
            local_app_data().map(|root| root.join("Programs").join("cursor").join("Cursor.exe")),
            local_app_data().map(|root| root.join("Programs").join("Cursor").join("Cursor.exe")),
            applications("Cursor.app"),
            which("cursor"),
        ]),
    );
    push_tool(
        &mut tools,
        "zcode",
        "agent",
        "ZCode",
        find_first(&[
            local_app_data().map(|root| root.join("Programs").join("ZCode").join("ZCode.exe")),
            applications("ZCode.app"),
            which("zcode"),
        ]),
    );
    push_tool(
        &mut tools,
        "opencode",
        "agent",
        "OpenCode",
        find_first(&[
            local_app_data()
                .map(|root| root.join("Programs").join("OpenCode").join("OpenCode.exe")),
            local_app_data().map(|root| root.join("OpenCode").join("OpenCode.exe")),
            applications("OpenCode.app"),
        ]),
    );
    push_tool(
        &mut tools,
        "kimi-desktop",
        "agent",
        "Kimi",
        find_first(&[
            local_app_data()
                .map(|root| root.join("Programs").join("kimi-desktop").join("Kimi.exe")),
            applications("Kimi.app"),
        ]),
    );
    push_tool(
        &mut tools,
        "antigravity",
        "agent",
        "Antigravity",
        find_first(&[
            local_app_data().map(|root| {
                root.join("Programs")
                    .join("Antigravity")
                    .join("Antigravity.exe")
            }),
            applications("Antigravity.app"),
            which("antigravity"),
        ]),
    );
    push_tool(
        &mut tools,
        "workbuddy",
        "agent",
        "WorkBuddy",
        find_first(&[
            local_app_data().map(|root| {
                root.join("Programs")
                    .join("WorkBuddy")
                    .join("WorkBuddy.exe")
            }),
            applications("WorkBuddy.app"),
        ]),
    );
    push_tool(
        &mut tools,
        "grok",
        "agent",
        "Grok",
        find_first(&[
            local_app_data()
                .map(|root| root.join("Programs").join("Grok Bot").join("Grok Bot.exe")),
            applications("Grok.app"),
        ]),
    );
    push_tool(
        &mut tools,
        "qoder",
        "agent",
        "Qoder",
        find_first(&[
            local_app_data().map(|root| root.join("Programs").join("Qoder").join("Qoder.exe")),
            applications("Qoder.app"),
            which("qoder"),
            which("qcode"),
        ]),
    );
    push_tool(
        &mut tools,
        "trae",
        "agent",
        "Trae",
        find_first(&[
            local_app_data().map(|root| root.join("Programs").join("Trae").join("Trae.exe")),
            applications("Trae.app"),
            which("trae"),
        ]),
    );
    push_tool(
        &mut tools,
        "windsurf",
        "agent",
        "Windsurf",
        find_first(&[
            local_app_data()
                .map(|root| root.join("Programs").join("Windsurf").join("Windsurf.exe")),
            applications("Windsurf.app"),
            which("windsurf"),
        ]),
    );
    push_cli_tool(
        &mut tools,
        "claude",
        "Claude Code",
        find_first(&[
            home_dir().map(|root| root.join(".local").join("bin").join("claude.exe")),
            home_dir().map(|root| root.join(".local").join("bin").join("claude")),
            applications("Claude.app"),
            which("claude"),
        ]),
    );
    push_cli_tool(
        &mut tools,
        "codex",
        "Codex",
        find_first(&[
            local_app_data().map(|root| {
                root.join("OpenAI")
                    .join("Codex")
                    .join("bin")
                    .join("codex.exe")
            }),
            which("codex"),
        ]),
    );
    push_cli_tool(
        &mut tools,
        "kimi",
        "Kimi Code",
        find_first(&[
            home_dir().map(|root| root.join(".kimi-code").join("bin").join("kimi.exe")),
            which("kimi"),
        ]),
    );
    push_cli_tool(
        &mut tools,
        "opencode-cli",
        "OpenCode CLI",
        find_first(&[
            local_app_data().map(|root| root.join("OpenCode").join("opencode-cli.exe")),
            which("opencode"),
        ]),
    );
    push_cli_tool(
        &mut tools,
        "gemini",
        "Gemini CLI",
        find_first(&[which("gemini")]),
    );
    push_cli_tool(
        &mut tools,
        "qwen",
        "Qwen Code",
        find_first(&[which("qwen")]),
    );
    push_cli_tool(
        &mut tools,
        "lingma",
        "Lingma",
        find_first(&[which("lingma")]),
    );
    tools
}

fn discover_ides() -> Vec<(ExternalTool, ToolBinary)> {
    let mut tools = Vec::new();
    push_tool(
        &mut tools,
        "cursor",
        "ide",
        "Cursor",
        find_first(&[
            local_app_data().map(|root| root.join("Programs").join("cursor").join("Cursor.exe")),
            local_app_data().map(|root| root.join("Programs").join("Cursor").join("Cursor.exe")),
            applications("Cursor.app"),
            which("cursor"),
        ]),
    );
    push_tool(
        &mut tools,
        "vscode",
        "ide",
        "VS Code",
        find_first(&[
            local_app_data().map(|root| {
                root.join("Programs")
                    .join("Microsoft VS Code")
                    .join("Code.exe")
            }),
            program_files().map(|root| root.join("Microsoft VS Code").join("Code.exe")),
            applications("Visual Studio Code.app"),
            which("code"),
        ]),
    );
    push_tool(
        &mut tools,
        "vscode-insiders",
        "ide",
        "VS Code Insiders",
        find_first(&[
            local_app_data().map(|root| {
                root.join("Programs")
                    .join("Microsoft VS Code Insiders")
                    .join("Code - Insiders.exe")
            }),
            applications("Visual Studio Code - Insiders.app"),
            which("code-insiders"),
        ]),
    );
    push_tool(
        &mut tools,
        "zed",
        "ide",
        "Zed",
        find_first(&[
            local_app_data().map(|root| root.join("Programs").join("Zed").join("zed.exe")),
            local_app_data().map(|root| root.join("Programs").join("Zed").join("Zed.exe")),
            applications("Zed.app"),
            which("zed"),
        ]),
    );
    push_tool(
        &mut tools,
        "visualstudio",
        "ide",
        "Visual Studio",
        find_visual_studio(),
    );
    for (id, name, binary) in jetbrains_ides() {
        push_tool(&mut tools, &id, "ide", &name, Some(binary));
    }
    push_tool(
        &mut tools,
        "xcode",
        "ide",
        "Xcode",
        applications("Xcode.app"),
    );
    tools
}

fn discover_terminals() -> Vec<(ExternalTool, ToolBinary)> {
    let mut tools = Vec::new();
    push_tool(
        &mut tools,
        "windows-terminal",
        "terminal",
        "Windows Terminal",
        find_first(&[
            local_app_data().map(|root| root.join("Microsoft").join("WindowsApps").join("wt.exe")),
            which("wt"),
        ]),
    );
    push_tool(
        &mut tools,
        "wezterm",
        "terminal",
        "WezTerm",
        find_first(&[
            which("wezterm-gui"),
            which("wezterm"),
            applications("WezTerm.app"),
        ]),
    );
    push_tool(
        &mut tools,
        "alacritty",
        "terminal",
        "Alacritty",
        find_first(&[which("alacritty"), applications("Alacritty.app")]),
    );
    push_tool(
        &mut tools,
        "tabby",
        "terminal",
        "Tabby",
        find_first(&[which("tabby"), applications("Tabby.app")]),
    );
    push_tool(
        &mut tools,
        "warp",
        "terminal",
        "Warp",
        find_first(&[which("warp"), applications("Warp.app")]),
    );
    push_tool(
        &mut tools,
        "iterm",
        "terminal",
        "iTerm",
        applications("iTerm.app"),
    );
    push_tool(
        &mut tools,
        "ghostty",
        "terminal",
        "Ghostty",
        applications("Ghostty.app"),
    );
    push_tool(
        &mut tools,
        "kitty",
        "terminal",
        "Kitty",
        find_first(&[which("kitty"), applications("kitty.app")]),
    );
    push_tool(
        &mut tools,
        "macos-terminal",
        "terminal",
        "Terminal",
        applications("Utilities/Terminal.app"),
    );
    push_tool(
        &mut tools,
        "git-bash",
        "terminal",
        "Git Bash",
        find_first(&[
            program_files().map(|root| root.join("Git").join("git-bash.exe")),
            program_files_x86().map(|root| root.join("Git").join("git-bash.exe")),
        ]),
    );
    push_tool(
        &mut tools,
        "pwsh",
        "terminal",
        "PowerShell",
        find_first(&[
            program_files().map(|root| root.join("PowerShell").join("7").join("pwsh.exe")),
            which("pwsh"),
        ]),
    );
    push_tool(
        &mut tools,
        "powershell",
        "terminal",
        "Windows PowerShell",
        find_first(&[
            Some(
                windows_system_root()
                    .join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe"),
            ),
            which("powershell"),
        ]),
    );
    push_tool(
        &mut tools,
        "cmd",
        "terminal",
        "Command Prompt",
        Some(windows_system_root().join("System32").join("cmd.exe")).filter(|path| path.exists()),
    );
    tools
}

fn push_tool(
    tools: &mut Vec<(ExternalTool, ToolBinary)>,
    id: &str,
    kind: &str,
    name: &str,
    program: Option<PathBuf>,
) {
    let Some(program) = program.filter(|path| path.exists() || is_app_bundle(path)) else {
        return;
    };
    if tools.iter().any(|(tool, _)| tool.id == id) {
        return;
    }
    tools.push((
        ExternalTool {
            id: id.into(),
            kind: kind.into(),
            name: name.into(),
        },
        ToolBinary {
            program,
            extra_args: Vec::new(),
            cli: false,
        },
    ));
}

fn push_cli_tool(
    tools: &mut Vec<(ExternalTool, ToolBinary)>,
    id: &str,
    name: &str,
    program: Option<PathBuf>,
) {
    let Some(program) = program.filter(|path| path.exists() || is_app_bundle(path)) else {
        return;
    };
    if tools.iter().any(|(tool, _)| tool.id == id) {
        return;
    }
    tools.push((
        ExternalTool {
            id: id.into(),
            kind: "agent".into(),
            name: name.into(),
        },
        ToolBinary {
            program,
            extra_args: Vec::new(),
            cli: true,
        },
    ));
}

fn spec_for_agent(binary: ToolBinary, dir: &Path) -> LaunchSpec {
    if binary.cli {
        return spec_for_cli_agent(binary, dir);
    }
    spec_for_ide(binary, dir)
}

fn spec_for_cli_agent(binary: ToolBinary, dir: &Path) -> LaunchSpec {
    #[cfg(target_os = "macos")]
    {
        return macos_cli_agent_spec(binary, dir);
    }
    #[cfg(not(target_os = "macos"))]
    {
        windows_cli_agent_spec(binary, dir)
    }
}

#[cfg(not(target_os = "macos"))]
fn windows_cli_agent_spec(binary: ToolBinary, dir: &Path) -> LaunchSpec {
    let program = binary.program;
    let quoted = quote_windows(&paths::path_to_string(&program));
    if let Some((_, wt)) = discover_terminals()
        .into_iter()
        .find(|(tool, _)| tool.id == "windows-terminal")
    {
        return LaunchSpec {
            program: wt.program,
            args: vec![
                OsString::from("-d"),
                dir.as_os_str().to_os_string(),
                OsString::from(quote_windows(&paths::path_to_string(&program))),
            ],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        };
    }
    if let Some((_, pwsh)) = discover_terminals()
        .into_iter()
        .find(|(tool, _)| tool.id == "pwsh" || tool.id == "powershell")
    {
        return LaunchSpec {
            program: pwsh.program,
            args: vec![
                OsString::from("-NoLogo"),
                OsString::from("-NoExit"),
                OsString::from("-Command"),
                OsString::from(format!(
                    "Set-Location -LiteralPath '{}'; & '{}'",
                    paths::path_to_string(dir).replace('\'', "''"),
                    paths::path_to_string(&program).replace('\'', "''")
                )),
            ],
            current_dir: Some(dir.to_path_buf()),
            windows_mode: WindowsMode::Console,
        };
    }
    LaunchSpec {
        program: windows_system_root().join("System32").join("cmd.exe"),
        args: vec![
            OsString::from("/D"),
            OsString::from("/K"),
            OsString::from(format!(
                "cd /d {} && {}",
                quote_windows(&paths::path_to_string(dir)),
                quoted
            )),
        ],
        current_dir: Some(dir.to_path_buf()),
        windows_mode: WindowsMode::Console,
    }
}

#[cfg(target_os = "macos")]
fn macos_cli_agent_spec(binary: ToolBinary, dir: &Path) -> LaunchSpec {
    let shell_command = format!(
        "cd {} && {}",
        shell_single_quote(&paths::path_to_string(dir)),
        shell_single_quote(&paths::path_to_string(&binary.program))
    );
    let script = format!(
        "tell application \"Terminal\"\n    do script \"{}\"\n    activate\nend tell",
        apple_script_string(&shell_command)
    );
    LaunchSpec {
        program: PathBuf::from("osascript"),
        args: vec![OsString::from("-e"), OsString::from(script)],
        current_dir: None,
        windows_mode: WindowsMode::Gui,
    }
}

#[cfg(target_os = "macos")]
fn shell_single_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(target_os = "macos")]
fn apple_script_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn spec_for_ide(binary: ToolBinary, dir: &Path) -> LaunchSpec {
    if is_app_bundle(&binary.program) {
        return LaunchSpec {
            program: PathBuf::from("open"),
            args: vec![
                OsString::from("-a"),
                binary.program.into_os_string(),
                OsString::from("--args"),
                dir.as_os_str().to_os_string(),
            ],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        };
    }
    wrap_windows_script(
        LaunchSpec {
            program: binary.program,
            args: {
                let mut args = binary.extra_args;
                args.push(dir.as_os_str().to_os_string());
                args
            },
            current_dir: Some(dir.to_path_buf()),
            windows_mode: WindowsMode::Gui,
        },
        dir,
    )
}

fn spec_for_terminal(id: &str, binary: ToolBinary, dir: &Path) -> LaunchSpec {
    if is_app_bundle(&binary.program) {
        return LaunchSpec {
            program: PathBuf::from("open"),
            args: vec![
                OsString::from("-a"),
                binary.program.into_os_string(),
                dir.as_os_str().to_os_string(),
            ],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        };
    }
    let dir_os = dir.as_os_str().to_os_string();
    match id {
        "windows-terminal" => LaunchSpec {
            program: binary.program,
            args: vec![OsString::from("-d"), dir_os],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        },
        "wezterm" => LaunchSpec {
            program: binary.program,
            args: vec![OsString::from("start"), OsString::from("--cwd"), dir_os],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        },
        "alacritty" => LaunchSpec {
            program: binary.program,
            args: vec![OsString::from("--working-directory"), dir_os],
            current_dir: None,
            windows_mode: WindowsMode::Gui,
        },
        "git-bash" => LaunchSpec {
            program: binary.program,
            args: vec![OsString::from(format!(
                "--cd={}",
                paths::path_to_string(dir)
            ))],
            current_dir: Some(dir.to_path_buf()),
            windows_mode: WindowsMode::Gui,
        },
        "pwsh" | "powershell" => LaunchSpec {
            program: binary.program,
            args: vec![
                OsString::from("-NoLogo"),
                OsString::from("-NoExit"),
                OsString::from("-Command"),
                OsString::from(format!(
                    "Set-Location -LiteralPath '{}'",
                    paths::path_to_string(dir).replace('\'', "''")
                )),
            ],
            current_dir: Some(dir.to_path_buf()),
            windows_mode: WindowsMode::Console,
        },
        "cmd" => LaunchSpec {
            program: binary.program,
            args: vec![
                OsString::from("/D"),
                OsString::from("/K"),
                OsString::from(format!(
                    "cd /d {}",
                    quote_windows(&paths::path_to_string(dir))
                )),
            ],
            current_dir: Some(dir.to_path_buf()),
            windows_mode: WindowsMode::Console,
        },
        _ => LaunchSpec {
            program: binary.program,
            args: binary.extra_args,
            current_dir: Some(dir.to_path_buf()),
            windows_mode: WindowsMode::Console,
        },
    }
}

fn wrap_windows_script(spec: LaunchSpec, dir: &Path) -> LaunchSpec {
    #[cfg(windows)]
    {
        let ext = spec
            .program
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "cmd" | "bat") {
            let mut args = vec![
                OsString::from("/D"),
                OsString::from("/C"),
                OsString::from("start"),
                OsString::from(""),
                OsString::from("/D"),
                dir.as_os_str().to_os_string(),
                spec.program.into_os_string(),
            ];
            args.extend(spec.args);
            return LaunchSpec {
                program: windows_system_root().join("System32").join("cmd.exe"),
                args,
                current_dir: Some(dir.to_path_buf()),
                windows_mode: WindowsMode::Gui,
            };
        }
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
    }
    spec
}

fn spawn_spec(spec: LaunchSpec) -> Result<()> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    if let Some(dir) = &spec.current_dir {
        command.current_dir(dir);
    }
    apply_windows_mode(&mut command, spec.windows_mode);
    command.spawn().map(|_| ()).map_err(|source| Error::Io {
        path: Some(spec.program),
        source,
    })
}

fn apply_windows_mode(command: &mut Command, mode: WindowsMode) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        match mode {
            WindowsMode::Console => {
                command.creation_flags(CREATE_NEW_CONSOLE);
            }
            WindowsMode::Gui => {}
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (command, mode);
    }
}

fn jetbrains_ides() -> Vec<(String, String, PathBuf)> {
    let mut found = Vec::new();
    let Some(scripts) =
        local_app_data().map(|root| root.join("JetBrains").join("Toolbox").join("scripts"))
    else {
        return found;
    };
    let products = [
        ("idea", "IntelliJ IDEA"),
        ("webstorm", "WebStorm"),
        ("pycharm", "PyCharm"),
        ("goland", "GoLand"),
        ("rustrover", "RustRover"),
        ("phpstorm", "PhpStorm"),
        ("clion", "CLion"),
        ("rider", "Rider"),
        ("rubymine", "RubyMine"),
        ("datagrip", "DataGrip"),
    ];
    for (id, name) in products {
        for ext in ["cmd", "bat", "exe"] {
            let candidate = scripts.join(format!("{id}.{ext}"));
            if candidate.exists() {
                found.push((format!("jetbrains-{id}"), name.to_string(), candidate));
                break;
            }
        }
    }
    if found.is_empty() {
        if let Some(idea) = find_first(&[applications("IntelliJ IDEA.app"), which("idea")]) {
            found.push(("jetbrains".into(), "JetBrains".into(), idea));
        }
    }
    found
}

fn find_visual_studio() -> Option<PathBuf> {
    let vswhere = program_files_x86().map(|root| {
        root.join("Microsoft Visual Studio")
            .join("Installer")
            .join("vswhere.exe")
    })?;
    if !vswhere.exists() {
        return which("devenv");
    }
    let mut command = Command::new(&vswhere);
    command.args(["-latest", "-property", "productPath"]);
    suppress_console_window(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = PathBuf::from(path);
    path.exists().then_some(path)
}

fn find_first(candidates: &[Option<PathBuf>]) -> Option<PathBuf> {
    candidates
        .iter()
        .flatten()
        .find(|path| path.exists() || is_app_bundle(path))
        .cloned()
}

fn which(name: &str) -> Option<PathBuf> {
    let requested = Path::new(name);
    if requested.is_absolute() {
        return requested.exists().then(|| requested.to_path_buf());
    }
    let exts = path_extensions(name);
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        for ext in &exts {
            let candidate = if ext.is_empty() {
                dir.join(name)
            } else {
                dir.join(format!("{name}{ext}"))
            };
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn path_extensions(name: &str) -> Vec<String> {
    if Path::new(name).extension().is_some() {
        return vec![String::new()];
    }
    #[cfg(windows)]
    {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .filter(|ext| !ext.is_empty())
            .map(str::to_string)
            .collect()
    }
    #[cfg(not(windows))]
    {
        vec![String::new()]
    }
}

fn local_app_data() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
}

fn program_files() -> Option<PathBuf> {
    env::var_os("ProgramFiles").map(PathBuf::from)
}

fn program_files_x86() -> Option<PathBuf> {
    env::var_os("ProgramFiles(x86)")
        .or_else(|| env::var_os("ProgramFiles"))
        .map(PathBuf::from)
}

fn windows_system_root() -> PathBuf {
    env::var_os("SystemRoot")
        .or_else(|| env::var_os("WINDIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:").join("Windows"))
}

fn applications(name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").map(PathBuf::from);
        let candidates = [
            Some(PathBuf::from("/Applications").join(name)),
            Some(PathBuf::from("/System/Applications").join(name)),
            home.as_ref()
                .map(|root| root.join("Applications").join(name)),
        ];
        return find_first(&candidates);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        None
    }
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("app") && path.exists()
}

fn quote_windows(value: &str) -> String {
    if value
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '&' | '^' | '(' | ')'))
    {
        let escaped = value.replace('"', r#""""#);
        format!(r#""{escaped}""#)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        quote_windows, resolve_project_dir, spec_for_agent, spec_for_cli_agent, spec_for_terminal,
        ToolBinary,
    };
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn resolve_project_dir_uses_parent_for_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("README.md");
        fs::write(&file, "ok").unwrap();
        let resolved = resolve_project_dir(&file.to_string_lossy()).unwrap();
        assert_eq!(resolved, crate::paths::canonicalize(dir.path()).unwrap());
    }

    #[test]
    fn missing_project_dir_is_an_error() {
        let err = resolve_project_dir("/this/path/should/not/exist-repoatlas").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("does not exist") || message.contains("path does not exist"));
    }

    #[test]
    fn cmd_terminal_stays_in_the_project_directory() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for_terminal(
            "cmd",
            ToolBinary {
                program: PathBuf::from("cmd.exe"),
                extra_args: Vec::new(),
                cli: false,
            },
            dir.path(),
        );
        assert_eq!(spec.current_dir.as_deref(), Some(dir.path()));
        let joined = spec
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("cd /d"));
    }

    #[test]
    fn windows_paths_with_spaces_are_quoted() {
        assert_eq!(
            quote_windows("C:\\Program Files\\Git"),
            "\"C:\\Program Files\\Git\""
        );
        assert_eq!(quote_windows("C:\\code\\repo"), "C:\\code\\repo");
    }

    #[test]
    fn cli_agent_launch_stays_in_the_project_directory() {
        let dir = tempfile::tempdir().unwrap();
        let spec = spec_for_cli_agent(
            ToolBinary {
                program: PathBuf::from("codex"),
                extra_args: Vec::new(),
                cli: true,
            },
            dir.path(),
        );
        let joined = spec
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            spec.current_dir.as_deref() == Some(dir.path())
                || joined.contains(&crate::paths::path_to_string(dir.path()))
        );
    }

    #[test]
    fn desktop_agent_launch_reuses_the_ide_spec() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("agent.exe");
        fs::write(&program, "ok").unwrap();
        let spec = spec_for_agent(
            ToolBinary {
                program: crate::paths::canonicalize(&program).unwrap(),
                extra_args: Vec::new(),
                cli: false,
            },
            dir.path(),
        );
        assert_eq!(spec.current_dir.as_deref(), Some(dir.path()));
    }

    #[test]
    #[cfg(any(windows, target_os = "macos"))]
    fn lists_at_least_one_terminal_on_supported_platforms() {
        let tools = super::list_external_tools();
        #[cfg(windows)]
        {
            assert!(tools.terminals.iter().any(|tool| tool.id == "cmd"
                || tool.id == "powershell"
                || tool.id == "windows-terminal"));
        }
        #[cfg(target_os = "macos")]
        {
            assert!(tools
                .terminals
                .iter()
                .any(|tool| tool.id == "macos-terminal" || tool.id == "iterm"));
        }
    }
}
