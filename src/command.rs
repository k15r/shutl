use crate::get_scripts_dir;
use crate::metadata::{ArgType, Config, LineType, parse_command_metadata};
use clap::{Arg, Command, crate_authors, crate_description, crate_name, crate_version};
use clap_complete::{ArgValueCompleter, CompletionCandidate, PathCompleter};
use is_executable::IsExecutable;
use shellexpand;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A command with its associated file path
pub struct CommandWithPath {
    pub command: Command,
    pub file_path: std::path::PathBuf,
}

/// Resolves the completion start directory from complete options.
/// Checks env var first, then falls back to the default path.
fn resolve_completion_dir(complete_options: &crate::metadata::CompleteOptions) -> Option<PathBuf> {
    // Check env var override first
    if let Some(ref env_var) = complete_options.env_var
        && let Ok(env_value) = std::env::var(env_var)
        && let Ok(expanded) = shellexpand::full(&env_value)
    {
        return Some(PathBuf::from(expanded.to_string()));
    }

    // Fall back to default path
    if let Some(path_str) = complete_options.path.to_str()
        && !path_str.is_empty()
        && let Ok(expanded) = shellexpand::full(path_str)
    {
        return Some(PathBuf::from(expanded.to_string()));
    }

    None
}

/// Adds a path completer to an argument based on its config
fn add_path_completer(arg: Arg, cfg: &Config) -> Arg {
    match &cfg.arg_type {
        Some(ArgType::Dir) => {
            let mut pc = PathCompleter::dir();
            if let Some(ref complete_options) = cfg.complete_options
                && let Some(dir) = resolve_completion_dir(complete_options)
            {
                pc = pc.current_dir(dir);
            }
            arg.add(ArgValueCompleter::new(pc))
        }
        Some(ArgType::File) => {
            let mut pc = PathCompleter::file();
            if let Some(ref complete_options) = cfg.complete_options
                && let Some(dir) = resolve_completion_dir(complete_options)
            {
                pc = pc.current_dir(dir);
            }
            arg.add(ArgValueCompleter::new(pc))
        }
        Some(ArgType::Path) => {
            let mut pc = PathCompleter::any();
            if let Some(ref complete_options) = cfg.complete_options
                && let Some(dir) = resolve_completion_dir(complete_options)
            {
                pc = pc.current_dir(dir);
            }
            arg.add(ArgValueCompleter::new(pc))
        }
        _ => arg,
    }
}

/// Builds a command for a script file
fn build_script_command(name: String, path: &Path) -> CommandWithPath {
    let metadata = parse_command_metadata(path);
    let mut cmd = Command::new(&name)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("shutlverboseid")
                .help("Print verbose information about the command")
                .long("shutl-verbose")
                .hide(true)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("shutlnoexec")
                .help(
                    "Do not execute the script, just print the command. Implies `--shutl-verbose`",
                )
                .hide(true)
                .long("shutl-noexec")
                .action(clap::ArgAction::SetTrue),
        );

    if !metadata.description.is_empty() {
        cmd = cmd.about(&metadata.description);
    }

    for cmdarg in &metadata.arguments {
        match cmdarg {
            LineType::Positional(name, description, cfg) => {
                let mut arg = Arg::new(name).help(description);
                arg = if let Some(ref default_value) = cfg.default {
                    arg.default_value(default_value.clone())
                } else {
                    arg.required(true)
                };
                if !cfg.options.is_empty() {
                    arg = arg.value_parser(clap::builder::PossibleValuesParser::new(&cfg.options))
                }

                if let Some(ArgType::CatchAll) = cfg.arg_type {
                    arg = arg.num_args(1..).action(clap::ArgAction::Append);
                    arg = arg.required(cfg.required);
                } else {
                    arg = add_path_completer(arg, cfg);
                }

                if cfg.required {
                    arg = arg.required(true);
                }

                cmd = cmd.arg(arg);
            }

            LineType::Flag(name, description, cfg) => {
                let mut arg = Arg::new(name).help(description).long(name);

                if let Some(ArgType::Bool) = cfg.arg_type {
                    let negated_name = format!("no-{}", name);
                    arg = arg
                        .action(clap::ArgAction::SetTrue)
                        .conflicts_with(&negated_name);
                    cmd = cmd.arg(
                        Arg::new(&negated_name)
                            .help(format!("Disable the '{}' flag", name))
                            .long(&negated_name)
                            .action(clap::ArgAction::SetTrue)
                            .conflicts_with(name),
                    );
                } else {
                    if let Some(ref default) = cfg.default {
                        arg = arg.default_value(default.clone());
                    }
                    if !cfg.options.is_empty() {
                        arg = arg
                            .value_parser(clap::builder::PossibleValuesParser::new(&cfg.options));
                    }
                }

                if cfg.required {
                    arg = arg.required(true);
                }

                arg = add_path_completer(arg, cfg);
                cmd = cmd.arg(arg);
            }
            _ => unreachable!(),
        }
    }

    CommandWithPath {
        command: cmd,
        file_path: path.to_path_buf(),
    }
}

/// Builds a list of commands from a directory
pub fn build_command_tree(dir_path: &Path, active_args: &[String]) -> Vec<CommandWithPath> {
    log::debug!(
        "build_command_tree: dir_path {:?}, active_args: {:?}",
        dir_path,
        active_args
    );
    let mut commands = Vec::new();
    let first_arg = active_args.first().cloned().unwrap_or_default();
    let rest = if active_args.is_empty() {
        &[]
    } else {
        &active_args[1..]
    };

    log::debug!(
        "build_command_tree: First arg: {:?}, active_args(rest): {:?}",
        first_arg,
        rest
    );

    if first_arg.is_empty() {
        return commands_for_dir(dir_path);
    }

    let first_arg_path = dir_path.join(&first_arg);
    log::debug!("build_command_tree: First arg path: {:?}", first_arg_path);

    if first_arg_path.is_dir() {
        let dir_name = first_arg_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let dir_cmd = add_dir_subcommands(
            dir_command(&first_arg_path, &dir_name),
            &first_arg_path,
            rest,
        );
        commands.push(CommandWithPath {
            command: dir_cmd,
            file_path: first_arg_path,
        });
        return commands;
    }

    if let Some(script_path) = find_script_file(dir_path, &first_arg) {
        commands.push(build_script_command(first_arg, &script_path));
        return commands;
    }

    build_command_tree(dir_path, rest)
}

fn add_dir_subcommands(
    mut dir_cmd: Command,
    first_arg_path: &Path,
    active_args: &[String],
) -> Command {
    for subcmd in build_command_tree(first_arg_path, active_args) {
        log::debug!(
            "build_command_tree: subcmd: {:?}",
            subcmd.command.get_name()
        );
        dir_cmd = dir_cmd.subcommand(subcmd.command);
    }
    dir_cmd
}

fn dir_command(path: &Path, dir_name: &String) -> Command {
    let mut dir_cmd = Command::new(dir_name).disable_help_subcommand(true);

    if let Ok(about) = fs::read_to_string(path.join(".shutl")) {
        dir_cmd = dir_cmd.about(about.trim().to_owned());
    }

    dir_cmd
}

fn commands_for_dir(dir: &Path) -> Vec<CommandWithPath> {
    let mut commands = Vec::new();
    log::debug!("commands_for_dir: {:?}", dir);

    if let Ok(entries) = fs::read_dir(dir) {
        let (mut directories, mut files): (Vec<_>, Vec<_>) = entries
            .filter_map(Result::ok)
            .partition(|entry| entry.path().is_dir());

        directories.retain(|entry| !entry.file_name().to_string_lossy().starts_with('.'));
        files.retain(|entry| {
            !entry.file_name().to_string_lossy().starts_with('.')
                && entry.path().is_file()
                && entry.path().is_executable()
        });

        let mut command_names = Vec::new();
        let mut use_extension = HashMap::new();

        for path in &directories {
            let dir_name = path.file_name().to_string_lossy().to_string();
            command_names.push(dir_name.clone());
            commands.push(CommandWithPath {
                command: dir_command(&path.path(), &dir_name),
                file_path: path.path(),
            });
        }

        for path in &files {
            let name = path.file_name().to_string_lossy().to_string();
            let clean_name = name.rsplitn(2, '.').last().unwrap_or(&name).to_string();
            if command_names.contains(&clean_name) {
                use_extension.insert(clean_name.clone(), true);
            } else {
                command_names.push(clean_name.clone());
            }
        }

        for path in files {
            let name = path.file_name().to_string_lossy().to_string();
            let clean_name = name.rsplitn(2, '.').last().unwrap_or(&name).to_string();
            let command_name = if use_extension.contains_key(&clean_name) {
                name
            } else {
                clean_name
            };
            commands.push(build_script_command(command_name, &path.path()));
        }
    }

    commands
}

fn find_script_file(dir_path: &Path, name: &str) -> Option<PathBuf> {
    let script_path = dir_path.join(name);
    if script_path.is_file() && script_path.is_executable() {
        return Some(script_path);
    }

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_file() && filename.rsplitn(2, ".").last().unwrap_or(&filename) == name {
                if path.is_executable() {
                    return Some(path);
                }
                return None;
            }
        }
    }

    None
}

/// Builds the complete CLI command structure
pub fn build_cli_command() -> Command {
    let args = std::env::args().collect::<Vec<_>>();
    let binary_with_path = std::env::args().next().unwrap_or_default();
    let binary_name = binary_with_path.rsplit('/').next().unwrap_or_default();
    let is_completion = std::env::var("_CLAP_COMPLETE_INDEX").is_ok()
        && args.get(1).is_some_and(|arg| arg == "--")
        && args.get(2).is_some_and(|arg| arg == binary_name);

    let active_args = if is_completion {
        args.into_iter().skip(2).collect::<Vec<_>>()
    } else {
        args
    };

    let mut cli = Command::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!())
        .disable_help_subcommand(true);

    // Add built-in commands
    cli = cli
        .subcommand(build_new_command())
        .subcommand(build_edit_command())
        .subcommand(build_list_command());

    for cmd_with_path in build_command_tree(&get_scripts_dir(), &active_args) {
        cli = cli.subcommand(cmd_with_path.command);
    }

    cli
}

/// Builds the 'new' subcommand for creating new scripts
pub fn build_new_command() -> Command {
    let scripts_dir = get_scripts_dir();
    Command::new("new")
        .about("Create a new script")
        .arg(
            Arg::new("location")
                .help("Location to create the script (relative to ~/.shutl)")
                .default_value("")
                .required(true)
                .add(ArgValueCompleter::new(
                    PathCompleter::dir().current_dir(scripts_dir),
                )),
        )
        .arg(
            Arg::new("name")
                .help("Name of the script (without .sh extension)")
                .required(true),
        )
        .arg(
            Arg::new("editor")
                .help("Editor to use (defaults to $EDITOR or 'vim')")
                .long("editor")
                .short('e'),
        )
        .arg(
            Arg::new("type")
                .help("Shell type for the script")
                .long("type")
                .short('t')
                .value_parser(clap::builder::PossibleValuesParser::new(vec![
                    "zsh", "bash",
                ]))
                .default_value("zsh"),
        )
        .arg(
            Arg::new("no-edit")
                .help("Don't open the script in an editor")
                .long("no-edit")
                .action(clap::ArgAction::SetTrue),
        )
}

/// Builds the 'edit' subcommand for editing existing scripts
pub fn build_edit_command() -> Command {
    Command::new("edit")
        .about("Edit an existing script")
        .arg(
            Arg::new("command")
                .help("Command path components (e.g., 'subdir myscript')")
                .required(true)
                .num_args(1..)
                .add(ArgValueCompleter::new(complete_script_names)),
        )
        .arg(
            Arg::new("editor")
                .help("Editor to use (defaults to $EDITOR or 'vim')")
                .long("editor")
                .short('e'),
        )
}

/// Builds the 'list' subcommand for listing available scripts
pub fn build_list_command() -> Command {
    let scripts_dir = get_scripts_dir();
    Command::new("list")
        .about("List available scripts")
        .arg(
            Arg::new("subdirectory")
                .help("Only list scripts under this subdirectory")
                .required(false)
                .add(ArgValueCompleter::new(
                    PathCompleter::dir().current_dir(scripts_dir),
                )),
        )
        .arg(
            Arg::new("tree")
                .help("Show hierarchical tree view")
                .long("tree")
                .action(clap::ArgAction::SetTrue),
        )
}

/// An entry representing a script found during listing
pub struct ListEntry {
    pub path: String,
    pub description: String,
}

/// Lists all scripts in the given directory, optionally filtered to a subdirectory.
/// Returns a formatted string ready for display.
pub fn list_scripts(base_dir: &Path, subdir_filter: Option<&str>, tree: bool) -> String {
    let normalized: Option<PathBuf> = subdir_filter.map(|s| Path::new(s).components().collect());
    let subdir_filter = normalized.as_deref().and_then(|p| p.to_str());
    let search_dir = if let Some(subdir) = subdir_filter {
        let p = base_dir.join(subdir);
        if !p.is_dir() {
            return format!("Directory not found: {}", subdir);
        }
        p
    } else {
        base_dir.to_path_buf()
    };

    let prefix = subdir_filter.unwrap_or("");
    let mut entries = Vec::new();
    collect_scripts(&search_dir, prefix, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    if entries.is_empty() {
        return "No scripts found.".to_string();
    }

    if tree {
        format_tree(&entries)
    } else {
        format_flat(&entries)
    }
}

fn collect_scripts(dir: &Path, prefix: &str, entries: &mut Vec<ListEntry>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    let (mut directories, mut files): (Vec<_>, Vec<_>) = read_dir
        .filter_map(Result::ok)
        .partition(|entry| entry.path().is_dir());

    directories.retain(|entry| !entry.file_name().to_string_lossy().starts_with('.'));
    directories.sort_by_key(|e| e.file_name());
    files.retain(|entry| {
        !entry.file_name().to_string_lossy().starts_with('.')
            && entry.path().is_file()
            && entry.path().is_executable()
    });
    files.sort_by_key(|e| e.file_name());

    for entry in &files {
        let name = entry.file_name().to_string_lossy().to_string();
        let clean_name = name.rsplitn(2, '.').last().unwrap_or(&name).to_string();
        let metadata = parse_command_metadata(&entry.path());
        let path = if prefix.is_empty() {
            clean_name
        } else {
            format!("{}/{}", prefix, clean_name)
        };
        entries.push(ListEntry {
            path,
            description: metadata.description,
        });
    }

    for entry in &directories {
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let sub_prefix = if prefix.is_empty() {
            dir_name.clone()
        } else {
            format!("{}/{}", prefix, dir_name)
        };
        collect_scripts(&entry.path(), &sub_prefix, entries);
    }
}

fn format_flat(entries: &[ListEntry]) -> String {
    let max_path_len = entries.iter().map(|e| e.path.len()).max().unwrap_or(0);
    entries
        .iter()
        .map(|e| {
            if e.description.is_empty() {
                e.path.clone()
            } else {
                format!(
                    "{:<width$}  {}",
                    e.path,
                    e.description,
                    width = max_path_len
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

use std::io::IsTerminal;

fn use_color() -> bool {
    std::io::stdout().is_terminal()
}

fn format_tree(entries: &[ListEntry]) -> String {
    let mut lines = Vec::new();
    let mut printed_dirs: Vec<String> = Vec::new();
    let color = use_color();
    let max_name_len = entries
        .iter()
        .map(|e| e.path.rsplit('/').next().unwrap_or(&e.path).len())
        .max()
        .unwrap_or(0);

    for entry in entries {
        let parts: Vec<&str> = entry.path.rsplitn(2, '/').collect();
        if parts.len() == 2 {
            let dir_path = parts[1];
            let name = parts[0];

            // Print any directory headers not yet printed
            let components: Vec<&str> = dir_path.split('/').collect();
            for depth in 0..components.len() {
                let ancestor: String = components[..=depth].join("/");
                if !printed_dirs.contains(&ancestor) {
                    let indent = "  ".repeat(depth);
                    let dir_label = if color {
                        format!("\x1b[1;34m{}/\x1b[0m", components[depth])
                    } else {
                        format!("{}/", components[depth])
                    };
                    lines.push(format!("{}{}", indent, dir_label));
                    printed_dirs.push(ancestor);
                }
            }

            let indent = "  ".repeat(components.len());
            let styled_name = if color {
                format!("\x1b[32m{}\x1b[0m", name)
            } else {
                name.to_string()
            };
            if entry.description.is_empty() {
                lines.push(format!("{}{}", indent, styled_name));
            } else {
                // Pad based on raw name length, then apply color
                let padding = max_name_len.saturating_sub(name.len());
                let desc = if color {
                    format!("\x1b[2m{}\x1b[0m", entry.description)
                } else {
                    entry.description.clone()
                };
                lines.push(format!(
                    "{}{}{}  {}",
                    indent,
                    styled_name,
                    " ".repeat(padding),
                    desc
                ));
            }
        } else {
            let styled_name = if color {
                format!("\x1b[32m{}\x1b[0m", entry.path)
            } else {
                entry.path.clone()
            };
            if entry.description.is_empty() {
                lines.push(styled_name.to_string());
            } else {
                let padding = max_name_len.saturating_sub(entry.path.len());
                let desc = if color {
                    format!("\x1b[2m{}\x1b[0m", entry.description)
                } else {
                    entry.description.clone()
                };
                lines.push(format!("{}{}  {}", styled_name, " ".repeat(padding), desc));
            }
        }
    }

    lines.join("\n")
}

/// Completer for script names in the edit command
fn complete_script_names(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    complete_script_names_in_dir(current, &get_scripts_dir())
}

/// Completer for script names in a given directory (testable version)
fn complete_script_names_in_dir(
    current: &std::ffi::OsStr,
    base_dir: &Path,
) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy();
    let parts: Vec<&str> = current_str.split('/').collect();

    // Build the path to search in
    let mut search_dir = base_dir.to_path_buf();
    if parts.len() > 1 {
        for part in &parts[..parts.len() - 1] {
            search_dir.push(part);
        }
    }

    let prefix = parts.last().unwrap_or(&"");
    let path_prefix = if parts.len() > 1 {
        parts[..parts.len() - 1].join("/") + "/"
    } else {
        String::new()
    };

    let mut completions = Vec::new();

    if let Ok(entries) = fs::read_dir(&search_dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            // Skip hidden files
            if name_str.starts_with('.') {
                continue;
            }

            let path = entry.path();

            if path.is_dir() {
                // Directory - add with trailing slash to indicate more completions
                if name_str.starts_with(prefix) {
                    completions.push(CompletionCandidate::new(format!(
                        "{}{}/",
                        path_prefix, name_str
                    )));
                }
            } else if path.is_file() && path.is_executable() {
                // Executable file - strip extension for completion
                let clean_name = name_str.rsplitn(2, '.').last().unwrap_or(&name_str);
                if clean_name.starts_with(prefix) {
                    completions.push(CompletionCandidate::new(format!(
                        "{}{}",
                        path_prefix, clean_name
                    )));
                }
            }
        }
    }

    completions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::find_script_file_in_dir;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn create_test_script(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let script_path = dir.join(name);
        let mut file = File::create(&script_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        // Set executable permissions
        fs::set_permissions(
            &script_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        script_path
    }

    #[test]
    fn test_build_script_command_full() {
        let script_content = r#"#!/bin/bash
#@description: test script
#@arg:pos - positional [required]
#@arg:pos-options - positional with options [options:!one!|two|three]
#@arg:pos-default - positional with default [default:default]
#@arg:pos-dir - positional with dir [dir:~/]
#@arg:pos-file - positional with file [file:~/]
#@arg:pos-any - positional with path [any:~/]
#@flag:flag - flag
#@flag:flag-bool - flag bool [bool]
#@flag:flag-bool-true - flag bool with default true [bool,default:true]
#@flag:flag-bool-false - flag bool with default false [bool,default:false]
#@flag:flag-dir - flag with dir [dir:~/]
#@flag:flag-file - flag with file [file:~/]
#@flag:flag-any - flag with path [any:~/]
#@flag:flag-options - flag with options [options:one|two|three]
#@flag:flag-options-default - flag with options and default [default:one, options:one|two|three]
#@flag:flag-options-default-exclamation - flag with options and default using exclamation mark [options:!one!|two|three]
#@flag:flag-required - flag required [required]
#@arg:... - Additional arguments
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        // Test command name
        assert_eq!(cmd_with_path.command.get_name(), "test");

        // Test description
        assert_eq!(
            cmd_with_path.command.get_about().unwrap().to_string(),
            "test script"
        );

        // Test arguments
        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        assert_eq!(args.len(), 23);

        validate_arg(&args, "pos", "positional", true, None, None);
        validate_arg(
            &args,
            "pos-default",
            "positional with default",
            false,
            Some("default".to_string()),
            None,
        );
        validate_arg(
            &args,
            "pos-options",
            "positional with options",
            false,
            Some("one".to_string()),
            Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ]),
        );
        validate_arg(&args, "pos-dir", "positional with dir", true, None, None);
        validate_arg(&args, "pos-file", "positional with file", true, None, None);
        validate_arg(&args, "pos-any", "positional with path", true, None, None);
        validate_arg(&args, "flag", "flag", false, None, None);
        validate_arg(&args, "flag-bool", "flag bool", false, None, None);
        validate_arg(
            &args,
            "flag-bool-true",
            "flag bool with default true",
            false,
            None,
            None,
        );
        validate_arg(
            &args,
            "flag-bool-false",
            "flag bool with default false",
            false,
            None,
            None,
        );
        validate_arg(
            &args,
            "no-flag-bool",
            "Disable the 'flag-bool' flag",
            false,
            None,
            None,
        );
        validate_arg(
            &args,
            "no-flag-bool-true",
            "Disable the 'flag-bool-true' flag",
            false,
            None,
            None,
        );
        validate_arg(
            &args,
            "no-flag-bool-false",
            "Disable the 'flag-bool-false' flag",
            false,
            None,
            None,
        );
        validate_arg(&args, "flag-dir", "flag with dir", false, None, None);
        validate_arg(&args, "flag-file", "flag with file", false, None, None);
        validate_arg(&args, "flag-any", "flag with path", false, None, None);
        validate_arg(
            &args,
            "flag-options",
            "flag with options",
            false,
            None,
            Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ]),
        );
        validate_arg(
            &args,
            "flag-options-default",
            "flag with options and default",
            false,
            Some("one".to_string()),
            Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ]),
        );
        validate_arg(
            &args,
            "flag-options-default-exclamation",
            "flag with options and default using exclamation mark",
            false,
            Some("one".to_string()),
            Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string(),
            ]),
        );
        validate_arg(&args, "flag-required", "flag required", true, None, None);
        validate_arg(
            &args,
            "additional-args",
            "Additional arguments",
            false,
            None,
            None,
        );
    }

    fn validate_arg(
        args: &[&Arg],
        name: &str,
        description: &str,
        is_required: bool,
        default: Option<String>,
        options: Option<Vec<String>>,
    ) {
        let arg = args.iter().find(|a| a.get_id() == name).unwrap();
        if is_required {
            assert!(arg.is_required_set());
        } else {
            assert!(!arg.is_required_set());
        }
        assert_eq!(arg.get_help().unwrap().to_string(), description);
        assert_eq!(arg.get_id(), name);
        if let Some(default) = default {
            let default_value = arg.get_default_values();
            assert_eq!(default_value.len(), 1);
            assert_eq!(default_value[0].to_str().unwrap(), default);
        }
        if let Some(options) = options {
            let possible_values = arg.get_possible_values();
            assert_eq!(possible_values.len(), options.len());
            for option in options {
                assert!(possible_values.iter().any(|v| v.get_name() == option));
            }
        }
    }

    #[test]
    fn test_build_script_command() {
        let script_content = r#"#!/bin/bash
#@description: Test command
#@arg:input - Input file
#@flag:verbose - Enable verbose output [bool]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        // Test command name
        assert_eq!(cmd_with_path.command.get_name(), "test");

        // Test description
        assert_eq!(
            cmd_with_path.command.get_about().unwrap().to_string(),
            "Test command"
        );

        // Test arguments
        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        assert_eq!(args.len(), 5); // input, verbose, no-verbose

        // Test input argument
        let input_arg = args.iter().find(|a| a.get_id() == "input").unwrap();
        assert!(input_arg.is_required_set());
        assert_eq!(input_arg.get_help().unwrap().to_string(), "Input file");

        // Test verbose flag
        let verbose_arg = args.iter().find(|a| a.get_id() == "verbose").unwrap();
        assert!(!verbose_arg.is_required_set());
        assert_eq!(
            verbose_arg.get_help().unwrap().to_string(),
            "Enable verbose output"
        );

        // Test no-verbose flag
        let no_verbose_arg = args.iter().find(|a| a.get_id() == "no-verbose").unwrap();
        assert!(!no_verbose_arg.is_required_set());
        assert_eq!(
            no_verbose_arg.get_help().unwrap().to_string(),
            "Disable the 'verbose' flag"
        );
    }

    #[test]
    fn test_bool_flag_conflicts() {
        let script_content = r#"#!/bin/bash
#@description: Test command
#@flag:verbose - Enable verbose output [bool]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        // Test that using both --verbose and --no-verbose results in an error
        let result = cmd_with_path.command.clone().try_get_matches_from(vec![
            "test",
            "--verbose",
            "--no-verbose",
        ]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_build_command_tree_for_subfolder() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");

        fs::create_dir(&scripts_dir).unwrap();

        // Create test directory structure
        // .shutl/subdir
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();

        // Create a subsubdirectory with a .shutl file
        // .shutl/subdir/test
        let subsubdir = subdir.join("test");
        fs::create_dir(&subsubdir).unwrap();
        let config_path = subsubdir.join(".shutl");
        fs::write(&config_path, "This is a test subsubdirectory").unwrap();
        // Create a script in the subsubdirectory
        // .shutl/subdir/test/subsubdir.sh
        create_test_script(
            &subsubdir,
            "subsubdir.sh",
            "#!/bin/bash\n#@description: Test script in subsubdirectory",
        );

        // Create test scripts
        // .shutl/subdir/test.sh
        create_test_script(
            &subdir,
            "test.sh",
            "#!/bin/bash\n#@description: subdir script",
        );

        let commands = build_command_tree(&scripts_dir, &["subdir".to_string()]);

        // Test root level command
        assert_eq!(commands.len(), 1); // root.sh and subdir
        // Test subdirectory command
        let subdir_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "subdir")
            .unwrap();
        let subdir_subcmds: Vec<_> = subdir_cmd.command.get_subcommands().collect();

        for subcmd in &subdir_subcmds {
            println!("Subcommand: {}", subcmd.get_name());
        }
        assert_eq!(subdir_subcmds.len(), 2);

        // Find the test command in the subdirectory
        let testdir_cmd = subdir_subcmds
            .iter()
            .find(|c| c.get_name() == "test")
            .unwrap();
        let testdir_subcmds: Vec<_> = testdir_cmd.get_subcommands().collect();
        assert_eq!(testdir_subcmds.len(), 0);
        // Find the test.sh command in the subdirectory
        let testscript_cmd = subdir_subcmds
            .iter()
            .find(|c| c.get_name() == "test.sh")
            .unwrap();
        let testscript_subcmds: Vec<_> = testscript_cmd.get_subcommands().collect();
        assert_eq!(testscript_subcmds.len(), 0);
    }

    #[test]
    fn test_build_command_tree_for_root() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create test directory structure
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();

        // Create test scripts
        create_test_script(
            &scripts_dir,
            "root.sh",
            "#!/bin/bash\n#@description: Root script",
        );
        create_test_script(&subdir, "sub.sh", "#!/bin/bash\n#@description: Sub script");

        let commands = build_command_tree(&scripts_dir, &[]);

        // Test root level command
        assert_eq!(commands.len(), 2); // root.sh and subdir
        let root_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "root")
            .unwrap();
        assert_eq!(
            root_cmd.command.get_about().unwrap().to_string(),
            "Root script"
        );

        // Test subdirectory command
        let subdir_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "subdir")
            .unwrap();
        let subdir_subcmds: Vec<_> = subdir_cmd.command.get_subcommands().collect();
        assert_eq!(subdir_subcmds.len(), 0);
    }

    #[test]
    fn test_build_command_tree_directory_description() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create a subdirectory
        let subdir = scripts_dir.join("test_dir");
        fs::create_dir(&subdir).unwrap();

        // Create a directory with a .shutl file
        let config_path = subdir.join(".shutl");
        fs::write(&config_path, "This is a test directory").unwrap();

        // Create a script in the directory
        create_test_script(
            &subdir,
            "test.sh",
            "#!/bin/bash\n#@description: Test script",
        );

        let commands = build_command_tree(&scripts_dir, &["test_dir".to_string()]);

        // Test directory command
        assert_eq!(commands.len(), 1); // only the directory command
        let dir_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "test_dir")
            .unwrap();
        assert_eq!(
            dir_cmd.command.get_about().unwrap().to_string(),
            "This is a test directory"
        );
    }

    #[test]
    fn test_build_command_tree_ignores_hidden() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create visible and hidden directories
        let visible_dir = scripts_dir.join("visible");
        let hidden_dir = scripts_dir.join(".hidden");
        fs::create_dir(&visible_dir).unwrap();
        fs::create_dir(&hidden_dir).unwrap();

        // Create visible and hidden scripts
        create_test_script(
            &scripts_dir,
            "visible_script.sh",
            "#!/bin/bash\n#@description: Visible script",
        );
        create_test_script(
            &scripts_dir,
            ".hidden_script.sh",
            "#!/bin/bash\n#@description: Hidden script",
        );

        let commands = build_command_tree(&scripts_dir, &[]);

        // Test that only visible items are included
        assert_eq!(commands.len(), 2); // visible.sh and visible directory
        let visible_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "visible")
            .unwrap();
        let visible_subcmds: Vec<_> = visible_cmd.command.get_subcommands().collect();
        assert_eq!(visible_subcmds.len(), 0);

        // Verify hidden items are not included
        assert!(
            commands
                .iter()
                .find(|c| c.command.get_name() == ".hidden")
                .is_none()
        );
        assert!(
            commands
                .iter()
                .find(|c| c.command.get_name() == ".hidden.sh")
                .is_none()
        );
    }

    #[test]
    fn test_new_command_script_names() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Test creating script without .sh extension
        let script1 = create_test_script(
            &scripts_dir,
            "test1.sh",
            "#!/bin/bash\n#@description: test1",
        );
        assert_eq!(script1.file_name().unwrap().to_str().unwrap(), "test1.sh");

        // Test creating script with .sh extension
        let script2 = create_test_script(
            &scripts_dir,
            "test2.sh.sh",
            "#!/bin/bash\n#@description: test2",
        );
        assert_eq!(
            script2.file_name().unwrap().to_str().unwrap(),
            "test2.sh.sh"
        );

        // Test creating script in subdirectory
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let script3 = create_test_script(&subdir, "test3.sh", "#!/bin/bash\n#@description: test3");
        assert_eq!(script3.file_name().unwrap().to_str().unwrap(), "test3.sh");

        // Verify all scripts are executable
        assert_ne!(script1.metadata().unwrap().permissions().mode() & 0o111, 0);
        assert_ne!(script2.metadata().unwrap().permissions().mode() & 0o111, 0);
        assert_ne!(script3.metadata().unwrap().permissions().mode() & 0o111, 0);

        // Verify script contents
        let content1 = fs::read_to_string(&script1).unwrap();
        assert!(content1.contains("#@description: test1"));
        let content2 = fs::read_to_string(&script2).unwrap();
        assert!(content2.contains("#@description: test2"));
        let content3 = fs::read_to_string(&script3).unwrap();
        assert!(content3.contains("#@description: test3"));
    }

    #[test]
    fn test_colliding_folder_and_script() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();
        // Create a directory with the same name as a script
        let subdir = scripts_dir.join("test");
        fs::create_dir(&subdir).unwrap();
        // Create a script with the same name as the directory
        create_test_script(
            &scripts_dir,
            "test.sh",
            "#!/bin/bash\n#@description: Test script",
        );

        // Create a script in the subdirectory to test collision
        create_test_script(
            &subdir,
            "subdirtest.sh",
            "#!/bin/bash\n#@description: Test script in subdirectory",
        );

        let commands = build_command_tree(&scripts_dir, &[]);

        // Verify both commands exist with different names
        assert_eq!(commands.len(), 2);

        // First script should have name "test"
        let cmd1 = commands
            .iter()
            .find(|c| c.command.get_name() == "test.sh")
            .unwrap();
        assert_eq!(cmd1.command.get_about().unwrap().to_string(), "Test script");
    }

    #[test]
    fn test_duplicate_script_names() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create two scripts with similar names
        create_test_script(
            &scripts_dir,
            "test.sh",
            "#!/bin/bash\n#@description: First test script",
        );
        create_test_script(
            &scripts_dir,
            "test.sh.sh",
            "#!/bin/bash\n#@description: Second test script",
        );

        let commands = build_command_tree(&scripts_dir, &[]);

        // Verify both commands exist with different names
        assert_eq!(commands.len(), 2);

        // First script should have name "test"
        let cmd1 = commands
            .iter()
            .find(|c| c.command.get_name() == "test")
            .unwrap();
        assert_eq!(
            cmd1.command.get_about().unwrap().to_string(),
            "First test script"
        );

        // Second script should have name "test.sh"
        let cmd2 = commands
            .iter()
            .find(|c| c.command.get_name() == "test.sh")
            .unwrap();
        assert_eq!(
            cmd2.command.get_about().unwrap().to_string(),
            "Second test script"
        );
    }

    #[test]
    fn test_edit_command() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create a nested directory structure
        let subdir = scripts_dir.join("subdir");
        let subsubdir = subdir.join("subsubdir");
        fs::create_dir_all(&subsubdir).unwrap();

        // Create test scripts at different levels
        let root_script = create_test_script(
            &scripts_dir,
            "root.sh",
            "#!/bin/bash\n#@description: Root script",
        );
        let sub_script =
            create_test_script(&subdir, "sub.sh", "#!/bin/bash\n#@description: Sub script");
        let subsub_script = create_test_script(
            &subsubdir,
            "subsub.sh",
            "#!/bin/bash\n#@description: Sub-sub script",
        );

        // Test editing root script
        let components = vec!["root".to_string()];
        assert_eq!(
            find_script_file_in_dir(&components, &scripts_dir).unwrap(),
            root_script
        );

        // Test editing script in subdirectory
        let components = vec!["subdir".to_string(), "sub".to_string()];
        assert_eq!(
            find_script_file_in_dir(&components, &scripts_dir).unwrap(),
            sub_script
        );

        // Test editing script in nested directory
        let components = vec![
            "subdir".to_string(),
            "subsubdir".to_string(),
            "subsub".to_string(),
        ];
        assert_eq!(
            find_script_file_in_dir(&components, &scripts_dir).unwrap(),
            subsub_script
        );

        // Test non-existent script
        let components = vec!["nonexistent".to_string()];
        assert!(find_script_file_in_dir(&components, &scripts_dir).is_none());

        // Test non-existent script in existing directory
        let components = vec!["subdir".to_string(), "nonexistent".to_string()];
        assert!(find_script_file_in_dir(&components, &scripts_dir).is_none());
    }

    #[test]
    fn test_complete_script_names_root_level() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        // Create test scripts and directories
        create_test_script(scripts_dir, "script1.sh", "#!/bin/bash");
        create_test_script(scripts_dir, "script2.sh", "#!/bin/bash");
        create_test_script(scripts_dir, "other.py", "#!/usr/bin/env python3");
        fs::create_dir(scripts_dir.join("subdir")).unwrap();

        // Create a hidden file that should be ignored
        let hidden_path = scripts_dir.join(".hidden.sh");
        fs::write(&hidden_path, "#!/bin/bash").unwrap();
        fs::set_permissions(&hidden_path, PermissionsExt::from_mode(0o755)).unwrap();

        // Test empty prefix - should return all scripts and directories
        let completions = complete_script_names_in_dir(std::ffi::OsStr::new(""), scripts_dir);
        let names: Vec<String> = completions
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"script1".to_string()));
        assert!(names.contains(&"script2".to_string()));
        assert!(names.contains(&"other".to_string()));
        assert!(names.contains(&"subdir/".to_string()));
        assert!(!names.iter().any(|n| n.contains(".hidden")));

        // Test prefix filtering
        let completions = complete_script_names_in_dir(std::ffi::OsStr::new("script"), scripts_dir);
        let names: Vec<String> = completions
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"script1".to_string()));
        assert!(names.contains(&"script2".to_string()));
        assert!(!names.contains(&"other".to_string()));
        assert!(!names.contains(&"subdir/".to_string()));
    }

    #[test]
    fn test_complete_script_names_nested() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        // Create nested structure
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();
        create_test_script(&subdir, "nested1.sh", "#!/bin/bash");
        create_test_script(&subdir, "nested2.sh", "#!/bin/bash");
        fs::create_dir(subdir.join("deeper")).unwrap();

        // Test completion in subdirectory
        let completions =
            complete_script_names_in_dir(std::ffi::OsStr::new("subdir/"), scripts_dir);
        let names: Vec<String> = completions
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"subdir/nested1".to_string()));
        assert!(names.contains(&"subdir/nested2".to_string()));
        assert!(names.contains(&"subdir/deeper/".to_string()));

        // Test prefix filtering in subdirectory
        let completions =
            complete_script_names_in_dir(std::ffi::OsStr::new("subdir/nested1"), scripts_dir);
        let names: Vec<String> = completions
            .iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"subdir/nested1".to_string()));
        assert!(!names.contains(&"subdir/nested2".to_string()));
    }

    #[test]
    fn test_complete_script_names_nonexistent_dir() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        // Test completion in non-existent directory returns empty
        let completions =
            complete_script_names_in_dir(std::ffi::OsStr::new("nonexistent/"), scripts_dir);
        assert!(completions.is_empty());
    }

    #[test]
    fn test_required_catchall_arg() {
        let script_content = r#"#!/bin/bash
#@description: Test required catch-all
#@arg:... - Additional arguments [required]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        let catchall = args
            .iter()
            .find(|a| a.get_id() == "additional-args")
            .unwrap();
        assert!(catchall.is_required_set());

        // Should fail without arguments
        let result = cmd_with_path
            .command
            .clone()
            .try_get_matches_from(vec!["test"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_optional_catchall_arg() {
        let script_content = r#"#!/bin/bash
#@description: Test optional catch-all
#@arg:... - Additional arguments
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        let catchall = args
            .iter()
            .find(|a| a.get_id() == "additional-args")
            .unwrap();
        assert!(!catchall.is_required_set());

        // Should succeed without arguments
        let result = cmd_with_path
            .command
            .clone()
            .try_get_matches_from(vec!["test"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_named_catchall_arg() {
        let script_content = r#"#!/bin/bash
#@description: Test named catch-all
#@arg:...files - Files to process [required]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        let catchall = args.iter().find(|a| a.get_id() == "files").unwrap();
        assert!(catchall.is_required_set());
        assert_eq!(catchall.get_help().unwrap().to_string(), "Files to process");

        // Should fail without arguments, error should mention "files"
        let result = cmd_with_path
            .command
            .clone()
            .try_get_matches_from(vec!["test"]);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("<files>"),
            "Error should mention 'files', got: {}",
            err_msg
        );
    }

    #[test]
    fn test_unnamed_catchall_defaults_to_additional_args() {
        let script_content = r#"#!/bin/bash
#@description: Test unnamed catch-all
#@arg:... - Extra args
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        let catchall = args
            .iter()
            .find(|a| a.get_id() == "additional-args")
            .unwrap();
        assert!(!catchall.is_required_set());
    }

    #[test]
    fn test_list_scripts_flat() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        // Create nested structure
        let docker_dir = scripts_dir.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        create_test_script(
            &docker_dir,
            "build.sh",
            "#!/bin/bash\n#@description: Build a Docker image",
        );
        create_test_script(
            &docker_dir,
            "push.sh",
            "#!/bin/bash\n#@description: Push image to registry",
        );

        create_test_script(
            scripts_dir,
            "hello.sh",
            "#!/bin/bash\n#@description: Say hello",
        );

        let output = list_scripts(scripts_dir, None, false);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("docker/build"));
        assert!(lines[0].contains("Build a Docker image"));
        assert!(lines[1].starts_with("docker/push"));
        assert!(lines[1].contains("Push image to registry"));
        assert!(lines[2].starts_with("hello"));
        assert!(lines[2].contains("Say hello"));
    }

    #[test]
    fn test_list_scripts_tree() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        let docker_dir = scripts_dir.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        create_test_script(
            &docker_dir,
            "build.sh",
            "#!/bin/bash\n#@description: Build a Docker image",
        );
        create_test_script(
            &docker_dir,
            "push.sh",
            "#!/bin/bash\n#@description: Push image to registry",
        );

        let compose_dir = docker_dir.join("compose");
        fs::create_dir(&compose_dir).unwrap();
        create_test_script(
            &compose_dir,
            "up.sh",
            "#!/bin/bash\n#@description: Start services",
        );
        create_test_script(
            &compose_dir,
            "down.sh",
            "#!/bin/bash\n#@description: Stop services",
        );

        create_test_script(
            scripts_dir,
            "hello.sh",
            "#!/bin/bash\n#@description: Say hello",
        );

        let output = list_scripts(scripts_dir, None, true);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 7);
        assert_eq!(lines[0], "docker/");
        assert!(lines[1].starts_with("  build"));
        assert_eq!(lines[2], "  compose/");
        assert!(lines[3].starts_with("    down"));
        assert!(lines[3].contains("Stop services"));
        assert!(lines[4].starts_with("    up"));
        assert!(lines[4].contains("Start services"));
        assert!(lines[5].starts_with("  push"));
        assert!(lines[5].contains("Push image to registry"));
        assert!(lines[6].starts_with("hello"));
        assert!(lines[6].contains("Say hello"));
    }

    #[test]
    fn test_list_scripts_filtered() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        let docker_dir = scripts_dir.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        create_test_script(
            &docker_dir,
            "build.sh",
            "#!/bin/bash\n#@description: Build a Docker image",
        );

        let k8s_dir = scripts_dir.join("k8s");
        fs::create_dir(&k8s_dir).unwrap();
        create_test_script(
            &k8s_dir,
            "deploy.sh",
            "#!/bin/bash\n#@description: Deploy to Kubernetes",
        );

        create_test_script(
            scripts_dir,
            "hello.sh",
            "#!/bin/bash\n#@description: Say hello",
        );

        let output = list_scripts(scripts_dir, Some("docker"), false);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("docker/build"));
        assert!(lines[0].contains("Build a Docker image"));
        // k8s and hello should NOT appear
        assert!(!output.contains("k8s"));
        assert!(!output.contains("hello"));
    }

    #[test]
    fn test_list_scripts_empty() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        let output = list_scripts(scripts_dir, None, false);
        assert_eq!(output, "No scripts found.");
    }

    #[test]
    fn test_list_scripts_nonexistent_subdir() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        let output = list_scripts(scripts_dir, Some("nonexistent"), false);
        assert_eq!(output, "Directory not found: nonexistent");
    }

    #[test]
    fn test_list_scripts_trailing_slash() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path();

        let docker_dir = scripts_dir.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        create_test_script(
            &docker_dir,
            "build.sh",
            "#!/bin/bash\n#@description: Build image",
        );

        let output = list_scripts(scripts_dir, Some("docker/"), false);
        assert!(output.contains("docker/build"));
        assert!(!output.contains("docker//build"));
    }
}
