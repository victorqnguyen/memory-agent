use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::json;

const CLAUDE_MD_SECTION: &str = "\n## Memory Agent
When you learn something important about a project (patterns, conventions, architecture decisions, bugs), save it with `memory_save`.\n";

fn claude_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
}

fn claude_md_path() -> PathBuf {
    claude_dir().join("CLAUDE.md")
}

fn commands_dir() -> PathBuf {
    claude_dir().join("commands")
}

fn hooks_dir() -> PathBuf {
    claude_dir().join("hooks").join("memory-agent")
}

fn binary_path() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

fn confirm(prompt: &str, auto_yes: bool) -> bool {
    if auto_yes {
        println!("{} [auto-yes]", prompt);
        return true;
    }
    print!("{} [y/N] ", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn read_input(prompt: &str, default: &str) -> String {
    if default.is_empty() {
        print!("{}: ", prompt);
    } else {
        print!("{} [{}]: ", prompt, default);
    }
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

pub enum LlmChoice {
    Ollama { url: String, model: String },
    None,
}

pub fn configure_llm(yes: bool) -> LlmChoice {
    println!("=== LLM Configuration ===\n");
    println!("memory-agent can use a local LLM for smarter memory consolidation,");
    println!("session summaries, and keyword extraction. Without one, it uses");
    println!("template-based processing (still works great).\n");
    println!("  [1] Local LLM via Ollama (recommended)");
    println!("      Uses Qwen3.5:2B — fast, private, runs on CPU");
    println!("  [2] None");
    println!("      Template-based processing, no additional setup\n");

    if yes {
        println!("Choice: 2 [auto-yes, skipping LLM setup]");
        return LlmChoice::None;
    }

    let choice = read_input("Choice", "1");

    match choice.as_str() {
        "2" => {
            println!("\n  Using template-based processing (no LLM).");
            println!("  You can enable Ollama later in ~/.memory-agent/config.toml\n");
            LlmChoice::None
        }
        _ => {
            println!("\n--- Ollama Setup ---\n");

            let ollama_installed = std::process::Command::new("ollama")
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success());

            let ollama_running = ollama_installed
                && std::process::Command::new("ollama")
                    .arg("list")
                    .output()
                    .is_ok_and(|o| o.status.success());

            if ollama_running {
                println!("  Ollama is installed and running.\n");
            } else {
                let install_cmd = if cfg!(target_os = "macos") {
                    "brew install ollama"
                } else {
                    "curl -fsSL https://ollama.com/install.sh | sh"
                };

                if !ollama_installed {
                    println!("  Ollama is not installed yet. To set it up:\n");
                    println!("    1. Install:  {}", install_cmd);
                    println!("    2. Start:    ollama serve");
                    println!("    3. Pull:     ollama pull qwen3.5:2b");
                } else {
                    println!("  Ollama is installed but not running. To set it up:\n");
                    println!("    1. Start:    ollama serve");
                    println!("    2. Pull:     ollama pull qwen3.5:2b");
                }
                println!("\n  You can do this now or after setup finishes.\n");
            }

            let model = read_input("Model", "qwen3.5:2b");
            let url = read_input("Ollama URL", "http://localhost:11434");
            println!();

            if ollama_running {
                println!("  Pulling model (this may take a minute on first run)...");
                let pull = std::process::Command::new("ollama")
                    .args(["pull", &model])
                    .status();
                match pull {
                    Ok(s) if s.success() => println!("  Model '{}' ready.\n", model),
                    Ok(_) => println!(
                        "  Warning: pull failed. Run 'ollama pull {}' manually.\n",
                        model
                    ),
                    Err(_) => println!(
                        "  Could not pull model. Run 'ollama pull {}' manually.\n",
                        model
                    ),
                }
            }

            println!("  You can change model/URL later: memory-agent config llm\n");

            LlmChoice::Ollama { url, model }
        }
    }
}

pub fn write_llm_config(data_dir: &Path, choice: &LlmChoice) -> anyhow::Result<()> {
    let config_path = data_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Find existing [llm] section and replace it
    let llm_start = lines.iter().position(|l| l.trim() == "[llm]");

    let new_section = match choice {
        LlmChoice::Ollama { url, model } => {
            let safe_url = url.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "");
            let safe_model = model.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "");
            format!(
                "[llm]\nollama_url = \"{}\"\nollama_model = \"{}\"\ntimeout_secs = 30",
                safe_url, safe_model
            )
        }
        LlmChoice::None => {
            "[llm]\n# ollama_url = \"http://localhost:11434\"\n# ollama_model = \"qwen3.5:2b\"\n# timeout_secs = 30".to_string()
        }
    };

    if let Some(start) = llm_start {
        // Find end of [llm] section (next [section] or EOF)
        let end = lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .find(|(_, l)| l.trim().starts_with('['))
            .map(|(i, _)| i)
            .unwrap_or(lines.len());

        // Remove old section, insert new
        lines.splice(start..end, new_section.lines().map(|l| l.to_string()));
    } else {
        // Append new section
        lines.push(String::new());
        lines.extend(new_section.lines().map(|l| l.to_string()));
    }

    // Ensure trailing newline
    let mut output = lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    std::fs::write(&config_path, output)?;
    Ok(())
}

pub fn configure_encryption(data_dir: &Path, yes: bool) -> anyhow::Result<()> {
    println!("=== Encryption ===\n");
    println!("memory-agent can encrypt your memory database at rest (SQLCipher).");
    println!("Recommended if your machine stores sensitive project information.");
    println!("The passphrase is stored in your system keychain.\n");
    println!("  [1] Enable encryption");
    println!("  [2] No encryption (default)\n");

    if yes {
        println!("Choice: 2 [auto-yes, skipping encryption]");
        return Ok(());
    }

    let choice = read_input("Choice", "2");

    if choice == "1" {
        // Encrypt existing DB if present — do this BEFORE generating/storing
        // a new passphrase so we never overwrite the keychain on failure.
        let db_path = data_dir.join("memory.db");
        if db_path.exists() {
            if memory_core::Store::is_encrypted(db_path.to_str().unwrap_or("")) {
                println!("  Database is already encrypted.");
            } else {
                println!("  Encrypting existing database...");
                let passphrase = crate::config_loader::generate_passphrase();
                let config = crate::config_loader::load(data_dir)
                    .map(|c| c.core)
                    .unwrap_or_default();
                memory_core::Store::encrypt(db_path.to_str().unwrap(), &passphrase, config)?;
                crate::config_loader::store_passphrase(&passphrase)?;
                println!("  Database encrypted. Passphrase stored in system keychain.");
            }
        } else {
            // No existing DB — generate and store passphrase now so it's
            // ready when the DB is first created by the MCP server.
            let passphrase = crate::config_loader::generate_passphrase();
            crate::config_loader::store_passphrase(&passphrase)?;
            println!("  Passphrase stored in system keychain.");
        }

        // Update config
        let config_path = data_dir.join("config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
            let mut found = false;
            for line in &mut lines {
                let trimmed = line.trim();
                if trimmed.starts_with("encryption_enabled")
                    || trimmed.starts_with("# encryption_enabled")
                {
                    *line = "encryption_enabled = true".to_string();
                    found = true;
                    break;
                }
            }
            if !found {
                if let Some(pos) = lines.iter().position(|l| l.trim() == "[storage]") {
                    lines.insert(pos + 1, "encryption_enabled = true".to_string());
                } else {
                    lines.push("[storage]".to_string());
                    lines.push("encryption_enabled = true".to_string());
                }
            }
            let mut output = lines.join("\n");
            if !output.ends_with('\n') {
                output.push('\n');
            }
            std::fs::write(&config_path, output)?;
        }

        println!("  Encryption enabled.");
    } else {
        println!(
            "\n  No encryption. You can enable it later: memory-agent config encryption enable\n"
        );
    }

    Ok(())
}

// --- Install ---

const OPENCODE_PLUGIN: &str = include_str!("../plugins/opencode-plugin.ts");

/// Key used by Cursor, Codex, Gemini config format.
const MCP_SERVERS_KEY: &str = "mcpServers";
/// Top-level key used by OpenCode config format. Servers nest directly: `mcp.<name>`.
const OPENCODE_MCP_KEY: &str = "mcp";
const MEMORY_SERVER_KEY: &str = "memory-agent";
const REGISTRY_KEY: &str = "_memory_agent";
const OPENCODE_PLUGIN_FILENAME: &str = "memory-agent.ts";

/// The hook events we manage. Written to settings.json on install so uninstall
/// knows exactly which events to touch — no path-sniffing heuristics needed.
const MANAGED_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PostToolUse",
    "PreCompact",
    "Stop",
    "TaskCompleted",
    "SubagentStop",
    "InstructionsLoaded",
    "SessionEnd",
];

fn build_registry() -> serde_json::Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "managed_events": MANAGED_EVENTS,
    })
}

/// Hook scripts embedded at compile time so `memory-agent install` works
/// from any directory, including after `cargo install`.
const HOOK_SCRIPTS: &[(&str, &str)] = &[
    (
        "session-start.sh",
        include_str!("../hooks/session-start.sh"),
    ),
    ("session-end.sh", include_str!("../hooks/session-end.sh")),
    ("user-prompt.sh", include_str!("../hooks/user-prompt.sh")),
    ("post-edit.sh", include_str!("../hooks/post-edit.sh")),
    ("pre-compact.sh", include_str!("../hooks/pre-compact.sh")),
    ("stop.sh", include_str!("../hooks/stop.sh")),
    (
        "task-completed.sh",
        include_str!("../hooks/task-completed.sh"),
    ),
    (
        "subagent-stop.sh",
        include_str!("../hooks/subagent-stop.sh"),
    ),
    (
        "instructions-loaded.sh",
        include_str!("../hooks/instructions-loaded.sh"),
    ),
    (
        "agent-review-gate.sh",
        include_str!("../hooks/agent-review-gate.sh"),
    ),
];

fn install_common(data_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let config_path = data_dir.join("config.toml");
    if !config_path.exists() {
        if confirm("Create default config?", yes) {
            crate::config_loader::create_default_config(data_dir)?;
            println!("  Created {}", config_path.display());
        }
    } else {
        println!("  Config already exists: {}", config_path.display());
    }

    println!();
    let llm_choice = configure_llm(yes);
    if config_path.exists() {
        write_llm_config(data_dir, &llm_choice)?;
        match &llm_choice {
            LlmChoice::Ollama { model, .. } => {
                println!("  LLM configured: Ollama with {}", model);
            }
            LlmChoice::None => {
                println!("  LLM: template-based (no Ollama)");
            }
        }
    }

    println!();
    configure_encryption(data_dir, yes)?;
    Ok(())
}

pub fn install(data_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let bin = binary_path().unwrap_or_else(|| PathBuf::from("memory-agent"));
    println!("=== memory-agent install ===\n");
    println!("Binary: {}", bin.display());
    println!("Data:   {}\n", data_dir.display());

    install_common(data_dir, yes)?;

    // 4. Register MCP server globally
    println!();
    if confirm(
        "Register as global MCP server (claude mcp add -s user)?",
        yes,
    ) {
        let status = std::process::Command::new("claude")
            .args(["mcp", "add", "-s", "user", "memory-agent", "--"])
            .arg(&bin)
            .arg("mcp")
            .status();
        match status {
            Ok(s) if s.success() => println!("  Registered MCP server globally."),
            Ok(s) => println!("  Warning: claude mcp add exited with {}", s),
            Err(e) => println!("  Warning: could not run 'claude': {}. Register manually:\n    claude mcp add -s user memory-agent -- {} mcp", e, bin.display()),
        }
    }

    // 5. Add CLAUDE.md section
    println!();
    let claude_md = claude_md_path();
    let has_section = claude_md.exists()
        && std::fs::read_to_string(&claude_md)
            .unwrap_or_default()
            .contains("## Memory Agent");
    if has_section {
        println!("  CLAUDE.md already has Memory Agent section.");
    } else if confirm("Add Memory Agent instructions to ~/.claude/CLAUDE.md?", yes) {
        std::fs::create_dir_all(claude_dir())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&claude_md)?;
        file.write_all(CLAUDE_MD_SECTION.as_bytes())?;
        println!("  Added Memory Agent section to {}", claude_md.display());
    }

    // 6. Install hook scripts (embedded in binary — works from any directory)
    println!();
    if confirm(
        "Install hook scripts to ~/.claude/hooks/memory-agent/?",
        yes,
    ) {
        let dest = hooks_dir();
        std::fs::create_dir_all(&dest)?;
        for (name, content) in HOOK_SCRIPTS {
            let dst = dest.join(name);
            std::fs::write(&dst, content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755))?;
            }
            println!("  Installed: {name}");
        }
        // 6b. Configure hooks in settings.json
        println!();
        if confirm("Configure hooks in ~/.claude/settings.json?", yes) {
            write_hook_config_to_settings()?;
        }
    }

    // 7. Copy slash commands
    println!();
    let src_commands = find_source_commands();
    if src_commands.is_empty() {
        println!("  No source slash commands found (run from project root to copy commands).");
    } else if confirm("Copy slash commands to ~/.claude/commands/?", yes) {
        let dest = commands_dir();
        std::fs::create_dir_all(&dest)?;
        let mut copied = 0;
        for src in &src_commands {
            let name = src.file_name().unwrap();
            let dst = dest.join(name);
            if dst.exists() {
                println!("  Skip (exists): {}", name.to_string_lossy());
            } else {
                std::fs::copy(src, &dst)?;
                println!("  Copied: {}", name.to_string_lossy());
                copied += 1;
            }
        }
        if copied == 0 {
            println!("  All commands already installed.");
        }
    }

    // Show MCP snippet for other agents the user might also use
    println!();
    print_mcp_snippet(&bin);
    print_system_prompt_instructions();

    println!(
        "\nInstall complete. Run 'memory-agent doctor' to verify, then start a new Claude session."
    );
    Ok(())
}

// --- Uninstall ---

pub fn uninstall(data_dir: &Path, purge: bool, yes: bool) -> anyhow::Result<()> {
    println!("=== memory-agent uninstall ===\n");

    let mut actions: Vec<String> = Vec::new();

    // Check what exists
    let claude_md = claude_md_path();
    let has_section = claude_md.exists()
        && std::fs::read_to_string(&claude_md)
            .unwrap_or_default()
            .contains("## Memory Agent");

    let installed_commands = find_installed_commands();
    let installed_hooks = find_installed_hooks();
    let data_exists = data_dir.exists();

    // Summarize what will be removed
    actions.push("Remove global MCP server registration".into());
    if has_section {
        actions.push(format!(
            "Remove Memory Agent section from {}",
            claude_md.display()
        ));
    }
    if !installed_hooks.is_empty() {
        actions.push(format!(
            "Remove {} hook scripts from {}",
            installed_hooks.len(),
            hooks_dir().display()
        ));
        actions.push("Remove hook config from ~/.claude/settings.json".into());
    }
    if !installed_commands.is_empty() {
        actions.push(format!(
            "Remove {} slash commands from {}",
            installed_commands.len(),
            commands_dir().display()
        ));
    }
    if purge && data_exists {
        actions.push(format!(
            "DELETE all data in {} (database, config)",
            data_dir.display()
        ));
    }

    if actions.is_empty() {
        println!("Nothing to uninstall.");
        return Ok(());
    }

    println!("The following actions will be performed:\n");
    for (i, action) in actions.iter().enumerate() {
        println!("  {}. {}", i + 1, action);
    }
    println!();

    if !confirm("Proceed with uninstall?", yes) {
        println!("Aborted.");
        return Ok(());
    }

    // 1. Remove MCP registration
    print!("Removing MCP registration... ");
    io::stdout().flush().ok();
    let status = std::process::Command::new("claude")
        .args(["mcp", "remove", "-s", "user", "memory-agent"])
        .status();
    match status {
        Ok(s) if s.success() => println!("done."),
        Ok(_) => println!("not found or already removed."),
        Err(e) => println!("skipped (claude not found: {}).", e),
    }

    // 2. Remove CLAUDE.md section
    if has_section {
        print!("Removing CLAUDE.md section... ");
        io::stdout().flush().ok();
        remove_claude_md_section(&claude_md)?;
        println!("done.");
    }

    // 3. Remove hook scripts + settings.json config
    if !installed_hooks.is_empty() {
        print!("Removing hook scripts... ");
        io::stdout().flush().ok();
        for hook in &installed_hooks {
            std::fs::remove_file(hook).ok();
        }
        println!("removed {}.", installed_hooks.len());

        print!("Removing hook config from settings.json... ");
        io::stdout().flush().ok();
        match remove_hook_config_from_settings() {
            Ok(true) => println!("done."),
            Ok(false) => println!("not found."),
            Err(e) => println!("skipped ({}).", e),
        }
    }

    // 4. Remove slash commands
    if !installed_commands.is_empty() {
        print!("Removing slash commands... ");
        io::stdout().flush().ok();
        for cmd in &installed_commands {
            std::fs::remove_file(cmd).ok();
        }
        println!("removed {}.", installed_commands.len());
    }

    // 5. Purge data
    if purge && data_exists {
        if !yes {
            println!();
            if !confirm(
                &format!(
                    "FINAL WARNING: Permanently delete {}? This cannot be undone.",
                    data_dir.display()
                ),
                false,
            ) {
                println!("Skipped data deletion. You can remove it manually later.");
                println!("\nUninstall complete (data preserved).");
                return Ok(());
            }
        }
        print!("Removing data directory... ");
        io::stdout().flush().ok();
        std::fs::remove_dir_all(data_dir)?;
        println!("done.");
    }

    println!("\nUninstall complete.");
    if !purge && data_exists {
        println!(
            "Data preserved at {}. Use --purge to remove it.",
            data_dir.display()
        );
    }
    Ok(())
}

fn remove_claude_md_section(path: &Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let mut result = String::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.starts_with("## Memory Agent") {
            in_section = true;
            continue;
        }
        if in_section {
            // End of section: next heading or separator that starts a new section
            if line.starts_with("## ") || (line.starts_with("---") && !result.is_empty()) {
                in_section = false;
                result.push_str(line);
                result.push('\n');
            }
            // Skip lines within the Memory Agent section
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    // Clean up leading/trailing whitespace but preserve content
    let trimmed = result.trim_start_matches('\n');
    std::fs::write(path, trimmed)?;
    Ok(())
}

fn find_source_commands() -> Vec<PathBuf> {
    // Look for commands relative to the binary's location or CWD
    let candidates = [
        std::env::current_dir()
            .ok()
            .map(|d| d.join(".claude/commands")),
        binary_path().and_then(|b| {
            b.parent()
                .and_then(|p| p.join("../../.claude/commands").canonicalize().ok())
        }),
    ];

    for candidate in candidates.iter().flatten() {
        if candidate.is_dir() {
            let mut found = Vec::new();
            if let Ok(entries) = std::fs::read_dir(candidate) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("memory-") && name.ends_with(".md") {
                        found.push(entry.path());
                    }
                }
            }
            if !found.is_empty() {
                return found;
            }
        }
    }
    Vec::new()
}

fn find_installed_hooks() -> Vec<PathBuf> {
    let mut found = Vec::new();

    // New namespaced location
    let dir = hooks_dir();
    if dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_memory_hook(&name) {
                    found.push(entry.path());
                }
            }
        }
    }

    // Old flat location (migration)
    let flat_dir = claude_dir().join("hooks");
    if flat_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&flat_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if is_memory_hook(&name) {
                    found.push(entry.path());
                }
            }
        }
    }

    found
}

fn is_memory_hook(name: &str) -> bool {
    matches!(
        name,
        "session-start.sh"
            | "session-end.sh"
            | "user-prompt.sh"
            | "post-edit.sh"
            | "pre-compact.sh"
            | "stop.sh"
            | "task-completed.sh"
            | "subagent-stop.sh"
            | "instructions-loaded.sh"
            | "agent-review-gate.sh"
    )
}

fn build_hook_config() -> serde_json::Value {
    let home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .to_string_lossy()
        .to_string();
    let h = |s: &str| format!("{home}/.claude/hooks/memory-agent/{s}");
    let cmd =
        |s: &str, t: u32| json!([{"hooks": [{"type": "command", "command": h(s), "timeout": t}]}]);
    let cmda = |s: &str| json!([{"hooks": [{"type": "command", "command": h(s), "async": true}]}]);
    let cmdm = |s: &str, m: &str, t: u32| json!([{"matcher": m, "hooks": [{"type": "command", "command": h(s), "timeout": t}]}]);

    json!({
        "SessionStart":       cmdm( "session-start.sh",       "startup|resume", 10),
        "UserPromptSubmit":   cmd(  "user-prompt.sh",         5),
        "PostToolUse":        [
            {"matcher": "Agent",      "hooks": [{"type": "command", "command": h("agent-review-gate.sh")}]},
            {"matcher": "Edit|Write", "hooks": [{"type": "command", "command": h("post-edit.sh"), "async": true}]},
        ],
        "PreCompact":         cmd(  "pre-compact.sh",         30),
        "Stop":               cmda( "stop.sh"),
        "TaskCompleted":      cmda( "task-completed.sh"),
        "SubagentStop":       cmda( "subagent-stop.sh"),
        "InstructionsLoaded": cmd(  "instructions-loaded.sh", 10),
        "SessionEnd":         cmd(  "session-end.sh",         10),
    })
}

/// Hook events that are exclusively ours — serialized as compact single-line values in
/// settings.json. PostToolUse is excluded because it may contain mixed user entries.
const COMPACT_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreCompact",
    "Stop",
    "TaskCompleted",
    "SubagentStop",
    "InstructionsLoaded",
    "SessionEnd",
];

/// Serialize settings.json with memory-agent hook events as compact single-line entries.
/// Non-memory-agent sections (e.g., Notification, PostToolUse) keep verbose formatting.
fn format_settings_json(root: &serde_json::Value) -> anyhow::Result<String> {
    use std::fmt::Write as FmtWrite;
    let obj = root
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("settings root is not an object"))?;
    let mut out = String::from("{\n");
    let len = obj.len();
    for (idx, (key, value)) in obj.iter().enumerate() {
        let comma = if idx + 1 < len { "," } else { "" };
        let key_json = serde_json::to_string(key)?;
        if key == "hooks" {
            writeln!(out, "  {key_json}: {{")?;
            if let Some(hooks_map) = value.as_object() {
                let hlen = hooks_map.len();
                for (hidx, (event, event_val)) in hooks_map.iter().enumerate() {
                    let hcomma = if hidx + 1 < hlen { "," } else { "" };
                    let event_json = serde_json::to_string(event)?;
                    if COMPACT_EVENTS.contains(&event.as_str()) {
                        let compact = serde_json::to_string(event_val)?;
                        writeln!(out, "    {event_json}: {compact}{hcomma}")?;
                    } else {
                        let pretty = serde_json::to_string_pretty(event_val)?;
                        let indented = indent_block(&pretty, "    ");
                        writeln!(out, "    {event_json}: {indented}{hcomma}")?;
                    }
                }
            }
            writeln!(out, "  }}{comma}")?;
        } else if key == REGISTRY_KEY {
            let compact = serde_json::to_string(value)?;
            writeln!(out, "  {key_json}: {compact}{comma}")?;
        } else {
            let pretty = serde_json::to_string_pretty(value)?;
            let indented = indent_block(&pretty, "  ");
            writeln!(out, "  {key_json}: {indented}{comma}")?;
        }
    }
    out.push('}');
    Ok(out)
}

fn indent_block(json: &str, prefix: &str) -> String {
    json.lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                line.to_string()
            } else {
                format!("{prefix}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pure: merge memory-agent hook config into an existing settings JSON value.
/// Extracted for testability — contains no IO.
fn apply_hook_config_to_json(root: &mut serde_json::Value) {
    let new_hooks = build_hook_config();
    if let Some(existing_hooks) = root.get_mut("hooks").and_then(|v| v.as_object_mut()) {
        // Merge: append our entries to existing event arrays
        if let Some(new_map) = new_hooks.as_object() {
            for (event, entries) in new_map {
                let arr = existing_hooks.entry(event).or_insert_with(|| json!([]));
                if let (Some(dst), Some(src)) = (arr.as_array_mut(), entries.as_array()) {
                    dst.extend(src.iter().cloned());
                }
            }
        }
    } else {
        root["hooks"] = new_hooks;
    }
    root[REGISTRY_KEY] = build_registry();
}

fn write_hook_config_to_settings() -> anyhow::Result<()> {
    let settings_path = claude_dir().join("settings.json");
    std::fs::create_dir_all(claude_dir())?;

    // Always remove old memory-agent hooks first (clean slate on reinstall)
    if let Err(e) = remove_hook_config_from_settings() {
        eprintln!("  Warning: could not remove existing hook config: {e}");
    }

    let mut root: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&raw).map_err(|e| {
            anyhow::anyhow!(
                "settings.json has invalid JSON: {e}\nPlease fix {} manually before re-running install.",
                settings_path.display()
            )
        })?
    } else {
        json!({})
    };

    apply_hook_config_to_json(&mut root);

    let json_str = format_settings_json(&root)?;
    std::fs::write(&settings_path, json_str)?;
    println!("  Wrote hook config to {}", settings_path.display());
    Ok(())
}

/// Pure: strip memory-agent hook entries from a settings JSON value.
/// Returns true if anything was removed. Extracted for testability — contains no IO.
fn strip_hook_config_from_json(root: &mut serde_json::Value) -> bool {
    // Read registry to know exactly which events are ours.
    // Falls back to path-sniffing for installs predating the registry.
    let had_registry = root.get(REGISTRY_KEY).is_some();
    let managed_events: Vec<String> = root
        .get(REGISTRY_KEY)
        .and_then(|r| r.get("managed_events"))
        .and_then(|e| e.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| MANAGED_EVENTS.iter().map(|s| s.to_string()).collect());

    // Old flat paths installed before the subdirectory namespace was introduced
    let old_scripts: &[&str] = &[
        "session-start.sh",
        "session-end.sh",
        "user-prompt.sh",
        "post-edit.sh",
        "pre-compact.sh",
        "stop.sh",
        "task-completed.sh",
        "subagent-stop.sh",
        "instructions-loaded.sh",
        "agent-review-gate.sh",
    ];

    let hooks = match root.get_mut("hooks").and_then(|v| v.as_object_mut()) {
        Some(h) => h,
        None => {
            root.as_object_mut().unwrap().remove(REGISTRY_KEY);
            return false;
        }
    };

    // Only touch our managed events — never scan or modify other events
    let mut removed_any = false;
    for event in &managed_events {
        if let Some(arr) = hooks.get_mut(event.as_str()).and_then(|v| v.as_array_mut()) {
            let before = arr.len();
            arr.retain(|entry| {
                let s = entry.to_string();
                if s.contains("/hooks/memory-agent/") {
                    return false;
                }
                // Migration: old flat paths under ~/.claude/hooks/<script>
                if old_scripts
                    .iter()
                    .any(|script| s.contains(&format!("/hooks/{script}")))
                    && s.contains("/.claude/hooks/")
                {
                    return false;
                }
                true
            });
            if arr.len() < before {
                removed_any = true;
            }
        }
    }

    if !removed_any && !had_registry {
        return false;
    }

    // Remove empty event arrays (only from our managed events)
    for event in &managed_events {
        if hooks
            .get(event.as_str())
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.is_empty())
        {
            hooks.remove(event.as_str());
        }
    }

    // If hooks object is now entirely empty, remove it
    if hooks.is_empty() {
        root.as_object_mut().unwrap().remove("hooks");
    }

    // Always remove registry key on uninstall
    root.as_object_mut().unwrap().remove(REGISTRY_KEY);

    true
}

fn remove_hook_config_from_settings() -> anyhow::Result<bool> {
    let settings_path = claude_dir().join("settings.json");
    if !settings_path.exists() {
        return Ok(false);
    }

    let raw = std::fs::read_to_string(&settings_path)?;
    let mut root: serde_json::Value = serde_json::from_str(&raw)?;

    let removed = strip_hook_config_from_json(&mut root);
    if removed {
        write_or_remove_json(&settings_path, &root)?;
    }
    Ok(removed)
}

// --- Multi-agent install/uninstall ---

pub enum AgentTarget {
    Gemini,
    Cursor,
    Codex,
    OpenCode,
}

impl AgentTarget {
    fn name(&self) -> &'static str {
        match self {
            AgentTarget::Gemini => "Gemini CLI",
            AgentTarget::Cursor => "Cursor",
            AgentTarget::Codex => "OpenAI Codex",
            AgentTarget::OpenCode => "OpenCode",
        }
    }

    fn config_path(&self) -> anyhow::Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
        Ok(match self {
            AgentTarget::Gemini => home.join(".gemini").join("settings.json"),
            AgentTarget::Cursor => home.join(".cursor").join("mcp.json"),
            AgentTarget::Codex => home.join(".codex").join("config.json"),
            AgentTarget::OpenCode => home.join(".config").join("opencode").join("opencode.json"),
        })
    }
}

pub fn install_agent(data_dir: &Path, yes: bool, target: AgentTarget) -> anyhow::Result<()> {
    let bin = binary_path().unwrap_or_else(|| PathBuf::from("memory-agent"));
    println!("=== memory-agent install ({}) ===\n", target.name());
    println!("Binary: {}", bin.display());
    println!("Data:   {}\n", data_dir.display());

    install_common(data_dir, yes)?;

    println!();
    let mcp_path = target.config_path()?;
    if confirm(&format!("Write MCP config to {}?", mcp_path.display()), yes) {
        write_mcp_json(&mcp_path, &bin)?;
        println!("  Wrote MCP config to {}", mcp_path.display());
    }

    // Show MCP snippet for other agents the user might also use
    println!();
    print_mcp_snippet(&bin);
    print_system_prompt_instructions();

    println!("\nInstall complete. Run 'memory-agent doctor' to verify.");
    Ok(())
}

pub fn uninstall_agent(purge: bool, yes: bool, target: AgentTarget) -> anyhow::Result<()> {
    if purge {
        println!("Note: --purge only removes shared data via 'uninstall claude --purge'.\n");
    }
    let mcp_path = target.config_path()?;
    println!("=== memory-agent uninstall ({}) ===\n", target.name());

    let mut root: serde_json::Value = match std::fs::read_to_string(&mcp_path) {
        Ok(raw) => serde_json::from_str(&raw)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "No config found at {}. Nothing to uninstall.",
                mcp_path.display()
            );
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    // Check for current key ("memory-agent") and legacy key ("memory") for migration
    let has_current = root
        .get(MCP_SERVERS_KEY)
        .and_then(|s| s.get(MEMORY_SERVER_KEY))
        .is_some();
    let has_legacy = root
        .get(MCP_SERVERS_KEY)
        .and_then(|s| s.get("memory"))
        .is_some();

    if !has_current && !has_legacy {
        println!(
            "No memory-agent entry in {}. Nothing to uninstall.",
            mcp_path.display()
        );
        return Ok(());
    }

    println!(
        "Will remove memory-agent from {} in {}\n",
        MCP_SERVERS_KEY,
        mcp_path.display()
    );

    if !confirm("Proceed with uninstall?", yes) {
        println!("Aborted.");
        return Ok(());
    }

    // Remove in-place from already-parsed JSON (both current and legacy keys)
    if let Some(servers) = root
        .get_mut(MCP_SERVERS_KEY)
        .and_then(|s| s.as_object_mut())
    {
        servers.remove(MEMORY_SERVER_KEY);
        // Only remove legacy "memory" key if it's ours
        if servers
            .get("memory")
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            .is_some_and(|cmd| cmd.contains("memory-agent"))
        {
            servers.remove("memory");
        }
        if servers.is_empty() {
            root.as_object_mut().unwrap().remove(MCP_SERVERS_KEY);
        }
    }

    write_or_remove_json(&mcp_path, &root)?;

    println!("Uninstall complete.");
    Ok(())
}

fn read_json_object(path: &Path) -> anyhow::Result<serde_json::Value> {
    match std::fs::read_to_string(path) {
        Ok(raw) if raw.trim().is_empty() => Ok(json!({})),
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(serde_json::Value::Object(obj)) => Ok(serde_json::Value::Object(obj)),
            Ok(_) => anyhow::bail!("{} exists but is not a JSON object", path.display()),
            Err(e) => anyhow::bail!("{} contains invalid JSON: {}", path.display(), e),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(e.into()),
    }
}

fn write_or_remove_json(path: &Path, root: &serde_json::Value) -> anyhow::Result<()> {
    if root.as_object().is_some_and(|o| o.is_empty()) {
        std::fs::remove_file(path).ok();
    } else {
        std::fs::write(path, serde_json::to_string_pretty(root)?)?;
    }
    Ok(())
}

fn write_mcp_json(config_path: &Path, bin: &Path) -> anyhow::Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root = read_json_object(config_path)?;
    let obj = root.as_object_mut().expect("root is always an object here");

    if !matches!(obj.get(MCP_SERVERS_KEY), Some(serde_json::Value::Object(_))) {
        obj.insert(MCP_SERVERS_KEY.into(), json!({}));
    }

    obj.get_mut(MCP_SERVERS_KEY)
        .and_then(|s| s.as_object_mut())
        .expect("mcpServers is always an object here")
        .insert(
            MEMORY_SERVER_KEY.to_string(),
            json!({
                "command": bin.to_string_lossy(),
                "args": ["mcp"]
            }),
        );

    std::fs::write(config_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

/// Removes the `mcp.memory-agent` entry from an OpenCode config file.
/// OpenCode format: servers nest directly under `mcp` (no `mcp.servers` indirection).
/// Cleans up empty `mcp` object. Writes the result back.
fn remove_opencode_mcp_entry(config_path: &Path) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(config_path)?;
    let mut root: serde_json::Value = serde_json::from_str(&raw)?;

    if let Some(mcp) = root
        .get_mut(OPENCODE_MCP_KEY)
        .and_then(|m| m.as_object_mut())
    {
        mcp.remove(MEMORY_SERVER_KEY);
        if mcp.is_empty() {
            root.as_object_mut().unwrap().remove(OPENCODE_MCP_KEY);
        }
    }

    write_or_remove_json(config_path, &root)
}

/// Writes the memory-agent MCP entry into OpenCode's config format.
///
/// OpenCode uses `mcp.<name>` with `type: "local"` and `command` as an array.
/// This is different from Cursor/Codex/Gemini which use `mcpServers.<name>`.
fn write_opencode_json(config_path: &Path, bin: &Path) -> anyhow::Result<()> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root = read_json_object(config_path)?;
    let obj = root.as_object_mut().expect("root is always an object here");

    if !matches!(
        obj.get(OPENCODE_MCP_KEY),
        Some(serde_json::Value::Object(_))
    ) {
        obj.insert(OPENCODE_MCP_KEY.into(), json!({}));
    }

    obj.get_mut(OPENCODE_MCP_KEY)
        .and_then(|m| m.as_object_mut())
        .expect("mcp is always an object here")
        .insert(
            MEMORY_SERVER_KEY.to_string(),
            json!({
                "type": "local",
                "command": [bin.to_string_lossy(), "mcp"]
            }),
        );

    std::fs::write(config_path, serde_json::to_string_pretty(&root)?)?;
    Ok(())
}

pub fn install_opencode(data_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let bin = binary_path().unwrap_or_else(|| PathBuf::from("memory-agent"));
    println!("=== memory-agent install (OpenCode) ===\n");
    println!("Binary: {}", bin.display());
    println!("Data:   {}\n", data_dir.display());
    println!("Note: OpenCode has built-in LSP support (rust-analyzer, etc.) —");
    println!("      no IDE required. Code intelligence works automatically.\n");

    install_common(data_dir, yes)?;

    println!();
    let config_path = AgentTarget::OpenCode.config_path()?;
    if confirm(
        &format!("Write MCP config to {}?", config_path.display()),
        yes,
    ) {
        write_opencode_json(&config_path, &bin)?;
        println!("  Written: {}", config_path.display());
    }

    // Plugin installation
    println!();
    let plugin_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
        .join(".config")
        .join("opencode")
        .join("plugins");
    if confirm(
        "Install OpenCode hook plugin (session tracking, auto-context)?",
        yes,
    ) {
        std::fs::create_dir_all(&plugin_dir)?;
        let plugin_path = plugin_dir.join("memory-agent.ts");
        std::fs::write(&plugin_path, OPENCODE_PLUGIN)?;
        println!("  Plugin installed: {}", plugin_path.display());
        println!("  Handles: session.created, file.edited, session.compacted, session.deleted");
    }

    println!();
    print_opencode_instructions();

    println!("\nInstall complete. Run 'memory-agent doctor' to verify.");
    Ok(())
}

fn print_opencode_instructions() {
    println!("Next steps:");
    println!("  1. Restart OpenCode to load the MCP server");
    println!("  2. Verify: run memory_list in an OpenCode session");
    println!("  3. Plugin handles session tracking automatically (if installed above)");
    println!();
    println!("OpenCode advantages over Claude Code:");
    println!("  - Built-in LSP (rust-analyzer, pyright, etc.) — starts automatically");
    println!("  - Model-agnostic: memories persist across any LLM provider");
    println!("  - MCP tools available to all configured models");
}

pub fn uninstall_opencode(purge: bool, yes: bool) -> anyhow::Result<()> {
    if purge {
        println!("Note: --purge only removes shared data via 'uninstall claude --purge'.\n");
    }
    let config_path = AgentTarget::OpenCode.config_path()?;
    println!("=== memory-agent uninstall (OpenCode) ===\n");

    let root: serde_json::Value = match std::fs::read_to_string(&config_path) {
        Ok(raw) => serde_json::from_str(&raw)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "No config found at {}. Nothing to uninstall.",
                config_path.display()
            );
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    let has_entry = root
        .get(OPENCODE_MCP_KEY)
        .and_then(|m| m.get(MEMORY_SERVER_KEY))
        .is_some();

    if !has_entry {
        println!("No memory-agent entry in mcp. Nothing to uninstall.");
        return Ok(());
    }

    println!(
        "Will remove {}.{} from {}\n",
        OPENCODE_MCP_KEY,
        MEMORY_SERVER_KEY,
        config_path.display()
    );

    if !confirm("Proceed with uninstall?", yes) {
        println!("Aborted.");
        return Ok(());
    }

    remove_opencode_mcp_entry(&config_path)?;

    // Also remove the plugin file if it exists
    let plugin_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
        .join(".config")
        .join("opencode")
        .join("plugins")
        .join(OPENCODE_PLUGIN_FILENAME);
    if plugin_path.exists() && confirm("Remove OpenCode hook plugin?", yes) {
        std::fs::remove_file(&plugin_path)?;
        println!("  Removed plugin: {}", plugin_path.display());
    }

    println!("Uninstall complete.");
    Ok(())
}

pub fn install_other(data_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let bin = binary_path().unwrap_or_else(|| PathBuf::from("memory-agent"));
    println!("=== memory-agent install (Other MCP Client) ===\n");
    println!("Binary: {}", bin.display());
    println!("Data:   {}\n", data_dir.display());

    install_common(data_dir, yes)?;

    println!();
    print_mcp_snippet(&bin);
    print_system_prompt_instructions();

    println!("\nInstall complete. Run 'memory-agent doctor' to verify.");
    Ok(())
}

fn print_mcp_snippet(bin: &Path) {
    let bin_str = bin.to_string_lossy();
    println!("=== MCP Configuration ===\n");
    println!("Add this to your MCP client's config file:\n");
    println!("{{");
    println!("  \"mcpServers\": {{");
    println!("    \"memory-agent\": {{");
    println!("      \"command\": \"{}\",", bin_str);
    println!("      \"args\": [\"mcp\"]");
    println!("    }}");
    println!("  }}");
    println!("}}");
    println!();
    println!("Common config file locations:");
    println!("  Windsurf:  ~/.codeium/windsurf/mcp_config.json");
    println!("  Zed:       ~/.config/zed/settings.json (under \"context_servers\")");
    println!("  Continue:  ~/.continue/config.json");
    println!("  Cline:     ~/.cline/mcp_settings.json");
    println!("  Generic:   Check your client's MCP documentation");
}

fn print_system_prompt_instructions() {
    println!("\n=== System Prompt Instructions ===\n");
    println!("Add these instructions to your agent's system prompt or rules file");
    println!("so it knows how to use memory-agent:\n");
    println!("---");
    print!("{}", CLAUDE_MD_SECTION.trim());
    println!("\n---");
}

fn find_installed_commands() -> Vec<PathBuf> {
    let dir = commands_dir();
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("memory-") && name.ends_with(".md") {
                found.push(entry.path());
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_opencode_json_creates_correct_structure() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let bin = std::path::PathBuf::from("/usr/local/bin/memory-agent");
        write_opencode_json(tmp.path(), &bin).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        // OpenCode: servers nest directly under "mcp", not "mcp.servers"
        let server = &v["mcp"]["memory-agent"];
        assert_eq!(server["type"], "local");
        assert_eq!(server["command"][0], "/usr/local/bin/memory-agent");
        assert_eq!(server["command"][1], "mcp");
        assert!(v.get("mcpServers").is_none(), "must NOT use mcpServers key");
        assert!(
            v["mcp"].get("servers").is_none(),
            "must NOT use mcp.servers nesting"
        );
    }

    #[test]
    fn test_write_opencode_json_merges_existing_config() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"theme": "dark"}"#).unwrap();
        write_opencode_json(tmp.path(), &std::path::PathBuf::from("memory-agent")).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["theme"], "dark");
        assert!(v["mcp"]["memory-agent"].is_object());
    }

    #[test]
    fn test_uninstall_opencode_removes_mcp_servers_entry() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Pre-populate with what write_opencode_json produces
        write_opencode_json(tmp.path(), &std::path::PathBuf::from("memory-agent")).unwrap();
        // Call the production removal helper directly
        remove_opencode_mcp_entry(tmp.path()).unwrap();

        // write_or_remove_json removes the file when root is empty — that's correct behaviour
        let v: serde_json::Value = match std::fs::read_to_string(tmp.path()) {
            Ok(s) => serde_json::from_str(&s).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert!(
            v.get(OPENCODE_MCP_KEY).is_none(),
            "mcp key should be removed when empty"
        );
        assert!(
            v.get(MCP_SERVERS_KEY).is_none(),
            "mcpServers was never written"
        );
    }

    #[test]
    fn test_opencode_plugin_embeds_correctly() {
        assert!(!OPENCODE_PLUGIN.is_empty());
        // All events handled through the single `event` hook with discriminated union
        assert!(OPENCODE_PLUGIN.contains("event: async ({ event })"));
        assert!(OPENCODE_PLUGIN.contains("session.created"));
        assert!(OPENCODE_PLUGIN.contains("file.edited"));
        assert!(OPENCODE_PLUGIN.contains("session.compacted"));
        assert!(OPENCODE_PLUGIN.contains("session.deleted"));
        assert!(OPENCODE_PLUGIN.contains("memory-agent"));
        // session.idle removed — context injection handled by MCP server
        assert!(!OPENCODE_PLUGIN.contains("session.idle"));
    }

    #[test]
    fn test_install_opencode_plugin_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let plugin_path = tmp.path().join("memory-agent.ts");
        std::fs::write(&plugin_path, OPENCODE_PLUGIN).unwrap();
        assert!(plugin_path.exists());
        let content = std::fs::read_to_string(&plugin_path).unwrap();
        assert!(content.contains("session.created"));
        assert!(content.contains("event: async ({ event })"));
    }

    // --- build_hook_config: Claude Code hook structure ---

    #[test]
    fn test_build_hook_config_has_all_nine_events() {
        let cfg = build_hook_config();
        let expected = [
            "SessionStart",
            "UserPromptSubmit",
            "PostToolUse",
            "PreCompact",
            "Stop",
            "TaskCompleted",
            "SubagentStop",
            "InstructionsLoaded",
            "SessionEnd",
        ];
        for event in &expected {
            assert!(cfg.get(event).is_some(), "missing hook event: {event}");
        }
    }

    #[test]
    fn test_build_hook_config_session_start_has_matcher_and_timeout() {
        let cfg = build_hook_config();
        let entry = &cfg["SessionStart"][0];
        assert_eq!(entry["matcher"], "startup|resume");
        let cmd = &entry["hooks"][0];
        assert_eq!(cmd["timeout"], 10);
        assert!(cmd["command"]
            .as_str()
            .unwrap()
            .contains("session-start.sh"));
    }

    #[test]
    fn test_build_hook_config_post_tool_use_has_two_matchers() {
        let cfg = build_hook_config();
        let entries = cfg["PostToolUse"].as_array().unwrap();
        assert_eq!(
            entries.len(),
            2,
            "PostToolUse must have exactly two entries"
        );
        let matchers: Vec<&str> = entries
            .iter()
            .filter_map(|e| e["matcher"].as_str())
            .collect();
        assert!(matchers.contains(&"Agent"), "missing Agent matcher");
        assert!(
            matchers.contains(&"Edit|Write"),
            "missing Edit|Write matcher"
        );
    }

    #[test]
    fn test_build_hook_config_post_tool_use_agent_is_blocking() {
        let cfg = build_hook_config();
        let agent_entry = cfg["PostToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["matcher"] == "Agent")
            .unwrap();
        let cmd = &agent_entry["hooks"][0];
        // blocking = no timeout, no async
        assert!(
            cmd.get("timeout").is_none(),
            "agent-review-gate must be blocking (no timeout)"
        );
        assert!(
            cmd.get("async").is_none(),
            "agent-review-gate must be blocking (no async)"
        );
        assert!(cmd["command"]
            .as_str()
            .unwrap()
            .contains("agent-review-gate.sh"));
    }

    #[test]
    fn test_build_hook_config_post_tool_use_edit_is_async() {
        let cfg = build_hook_config();
        let edit_entry = cfg["PostToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["matcher"] == "Edit|Write")
            .unwrap();
        let cmd = &edit_entry["hooks"][0];
        assert_eq!(cmd["async"], true);
        assert!(cmd["command"].as_str().unwrap().contains("post-edit.sh"));
    }

    #[test]
    fn test_build_hook_config_async_events() {
        let cfg = build_hook_config();
        for event in &["Stop", "TaskCompleted", "SubagentStop"] {
            let cmd = &cfg[*event][0]["hooks"][0];
            assert_eq!(cmd["async"], true, "{event} must be async");
        }
    }

    #[test]
    fn test_build_hook_config_timeouts() {
        let cfg = build_hook_config();
        assert_eq!(cfg["UserPromptSubmit"][0]["hooks"][0]["timeout"], 5);
        assert_eq!(cfg["PreCompact"][0]["hooks"][0]["timeout"], 30);
        assert_eq!(cfg["InstructionsLoaded"][0]["hooks"][0]["timeout"], 10);
        assert_eq!(cfg["SessionEnd"][0]["hooks"][0]["timeout"], 10);
    }

    #[test]
    fn test_build_hook_config_all_commands_under_hooks_memory_agent_dir() {
        let cfg = build_hook_config();
        // Collect every command string from every hook entry recursively
        fn commands(v: &serde_json::Value) -> Vec<String> {
            match v {
                serde_json::Value::Array(arr) => arr.iter().flat_map(commands).collect(),
                serde_json::Value::Object(map) => {
                    if let Some(cmd) = map.get("command").and_then(|c| c.as_str()) {
                        vec![cmd.to_string()]
                    } else {
                        map.values().flat_map(commands).collect()
                    }
                }
                _ => vec![],
            }
        }
        for (event, val) in cfg.as_object().unwrap() {
            for cmd in commands(val) {
                assert!(
                    cmd.contains("hooks/memory-agent/"),
                    "{event}: command '{cmd}' is not under hooks/memory-agent/"
                );
            }
        }
    }

    // --- write_mcp_json: Cursor / Gemini / Codex MCP config ---

    #[test]
    fn test_write_mcp_json_creates_correct_structure() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let bin = PathBuf::from("/usr/local/bin/memory-agent");
        write_mcp_json(tmp.path(), &bin).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
        let server = &v[MCP_SERVERS_KEY][MEMORY_SERVER_KEY];
        assert_eq!(server["command"], "/usr/local/bin/memory-agent");
        assert_eq!(server["args"][0], "mcp");
    }

    #[test]
    fn test_write_mcp_json_merges_existing_config() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            r#"{"mcpServers": {"other-tool": {"command": "other"}}}"#,
        )
        .unwrap();
        write_mcp_json(tmp.path(), &PathBuf::from("memory-agent")).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
        // Existing entry preserved
        assert_eq!(v[MCP_SERVERS_KEY]["other-tool"]["command"], "other");
        // New entry added
        assert!(v[MCP_SERVERS_KEY][MEMORY_SERVER_KEY].is_object());
    }

    #[test]
    fn test_write_mcp_json_idempotent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let bin = PathBuf::from("memory-agent");
        write_mcp_json(tmp.path(), &bin).unwrap();
        write_mcp_json(tmp.path(), &bin).unwrap(); // second call
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
        // Must not create duplicate keys — mcpServers.memory-agent appears exactly once
        let servers = v[MCP_SERVERS_KEY].as_object().unwrap();
        assert_eq!(servers.len(), 1);
    }

    // --- settings.json: apply + strip hook config (Claude Code) ---

    #[test]
    fn test_apply_hook_config_to_empty_json() {
        let mut root = json!({});
        apply_hook_config_to_json(&mut root);
        // All 9 events present
        let hooks = root["hooks"].as_object().unwrap();
        for event in &[
            "SessionStart",
            "UserPromptSubmit",
            "PostToolUse",
            "PreCompact",
            "Stop",
            "TaskCompleted",
            "SubagentStop",
            "InstructionsLoaded",
            "SessionEnd",
        ] {
            assert!(hooks.contains_key(*event), "missing {event} after apply");
        }
        // Registry written
        assert!(root.get(REGISTRY_KEY).is_some(), "registry key missing");
    }

    #[test]
    fn test_apply_hook_config_merges_with_existing_user_hooks() {
        let mut root = json!({
            "hooks": {
                "PostToolUse": [{"matcher": "MyTool", "hooks": [{"type": "command", "command": "/user/custom.sh"}]}]
            }
        });
        apply_hook_config_to_json(&mut root);
        let post_tool = root["hooks"]["PostToolUse"].as_array().unwrap();
        // User entry preserved, our two entries appended = 3 total
        assert_eq!(
            post_tool.len(),
            3,
            "user hook must be preserved alongside our two entries"
        );
        assert!(
            post_tool[0]["matcher"] == "MyTool",
            "user entry should remain first"
        );
    }

    #[test]
    fn test_strip_hook_config_removes_our_entries() {
        let mut root = json!({});
        apply_hook_config_to_json(&mut root);
        // Verify entries exist first
        assert!(root["hooks"]["SessionStart"].as_array().unwrap().len() > 0);

        let removed = strip_hook_config_from_json(&mut root);
        assert!(removed, "should report removal");
        // All hook events removed, hooks object itself gone
        assert!(root.get("hooks").is_none(), "hooks key should be gone");
        assert!(
            root.get(REGISTRY_KEY).is_none(),
            "registry key should be gone"
        );
    }

    #[test]
    fn test_strip_hook_config_preserves_user_hooks() {
        let user_cmd = "/home/user/my-custom-hook.sh";
        let mut root = json!({
            "hooks": {
                "PostToolUse": [{"matcher": "MyTool", "hooks": [{"type": "command", "command": user_cmd}]}]
            }
        });
        apply_hook_config_to_json(&mut root);
        strip_hook_config_from_json(&mut root);

        // User's PostToolUse entry must survive
        let post_tool = root["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(post_tool.len(), 1, "only the user entry should remain");
        assert!(post_tool[0].to_string().contains(user_cmd));
    }

    #[test]
    fn test_strip_hook_config_returns_false_when_nothing_to_remove() {
        let mut root = json!({"otherKey": true});
        let removed = strip_hook_config_from_json(&mut root);
        assert!(!removed);
    }

    #[test]
    fn test_apply_then_strip_is_clean_roundtrip() {
        let original = json!({"model": "claude-opus-4-5", "theme": "dark"});
        let mut root = original.clone();
        apply_hook_config_to_json(&mut root);
        strip_hook_config_from_json(&mut root);
        // Should be back to original (no leftover keys)
        assert_eq!(root, original);
    }
}
