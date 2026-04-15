//! Built-in subcommands: new, edit, list, validate.

use clap::ArgMatches;
use std::path::Path;
use std::process::Command;

use crate::command::{build_script_command_for_help, list_scripts};
use crate::validation::{
    Severity, format_diagnostics, format_diagnostics_as_comments, has_errors, validate_script,
};
use crate::{find_script_file, get_scripts_dir, resolve_editor};

/// Create a new script under the scripts directory.
pub fn handle_new(new_matches: &ArgMatches) {
    let name = new_matches.get_one::<String>("name").unwrap();
    let location = new_matches.get_one::<String>("location").unwrap();
    let editor = new_matches.get_one::<String>("editor");
    let no_edit = new_matches.get_flag("no-edit");
    let script_type = new_matches
        .get_one::<String>("type")
        .map(|s| s.as_str())
        .unwrap_or("zsh");

    // Build the script path
    let mut script_path = get_scripts_dir();
    if !location.is_empty() {
        script_path.push(location);
    }

    let with_extension = format!("{}.sh", name);
    let script_name = if name.contains('.') {
        name.to_string()
    } else {
        with_extension
    };
    script_path.push(&script_name);

    // Ensure parent directories exist
    if let Some(parent) = script_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Failed to create directory {}: {}", parent.display(), e);
        std::process::exit(1);
    }

    let shebang = match script_type {
        "bash" => "#!/bin/bash",
        _ => "#!/bin/zsh",
    };

    // Write the script template
    let template = format!(
        "{}\n#@description: {}\n#@arg:input - Input file\n#@flag:verbose - Enable verbose output\n",
        shebang,
        name.trim_end_matches(".sh"),
    );

    if let Err(e) = std::fs::write(&script_path, template) {
        eprintln!("Failed to write script {}: {}", script_path.display(), e);
        std::process::exit(1);
    }

    // Make the script executable
    if let Err(e) = std::fs::set_permissions(
        &script_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    ) {
        eprintln!(
            "Failed to set permissions on {}: {}",
            script_path.display(),
            e
        );
        std::process::exit(1);
    }

    // Open the script in an editor if required
    if !no_edit {
        let editor = resolve_editor(editor);

        Command::new(editor)
            .arg(&script_path)
            .status()
            .expect("Failed to open editor");
    }

    println!("Created script: {}", script_path.display());
}

/// Edit an existing script by path components, with post-edit validation.
/// If validation fails, the user is dropped back into the editor with error
/// comments prepended (similar to `kubectl edit`).
pub fn handle_edit(edit_matches: &ArgMatches) {
    let raw_components: Vec<String> = edit_matches
        .get_many::<String>("command")
        .unwrap()
        .map(|s| s.to_string())
        .collect();

    let components: Vec<String> = raw_components
        .iter()
        .flat_map(|s| s.split('/'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let editor = edit_matches.get_one::<String>("editor");

    if let Some(script_path) = find_script_file(&components) {
        let editor = resolve_editor(editor);
        edit_with_validation(&script_path, &editor);
        println!("Edited script: {}", script_path.display());
    } else {
        eprintln!("Script not found: {}", components.join("/"));
        std::process::exit(1);
    }
}

/// Opens the script in an editor, then validates. On validation errors,
/// prepends error comments and reopens (loop until valid or user aborts).
fn edit_with_validation(script_path: &Path, editor: &str) {
    let original_content =
        std::fs::read_to_string(script_path).expect("Failed to read script file");

    Command::new(editor)
        .arg(script_path)
        .status()
        .expect("Failed to open editor");

    loop {
        let diagnostics = validate_script(script_path);
        if !has_errors(&diagnostics) {
            if !diagnostics.is_empty() {
                eprintln!("{}", format_diagnostics(&diagnostics));
            }
            return;
        }

        eprintln!("\nValidation failed:\n{}", format_diagnostics(&diagnostics));

        let current_content =
            std::fs::read_to_string(script_path).expect("Failed to read script file");
        let stripped = strip_validation_comments(&current_content);
        let error_block = format_diagnostics_as_comments(&diagnostics);
        let annotated = insert_validation_comments(&stripped, &error_block);

        std::fs::write(script_path, &annotated).expect("Failed to write annotated script");

        Command::new(editor)
            .arg(script_path)
            .status()
            .expect("Failed to open editor");

        let after_edit = std::fs::read_to_string(script_path).expect("Failed to read script file");

        if after_edit == annotated {
            eprintln!("No changes made, restoring original and aborting edit.");
            std::fs::write(script_path, &original_content)
                .expect("Failed to restore original script");
            std::process::exit(1);
        }

        let cleaned = strip_validation_comments(&after_edit);
        std::fs::write(script_path, &cleaned).expect("Failed to write cleaned script");
    }
}

const VALIDATION_MARKER: &str = "# ===========================================================";

fn strip_validation_comments(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let first_marker = lines.iter().position(|l| *l == VALIDATION_MARKER);
    let last_marker = lines.iter().rposition(|l| *l == VALIDATION_MARKER);

    let kept: Vec<&str> = match (first_marker, last_marker) {
        (Some(first), Some(last)) if first != last => lines[..first]
            .iter()
            .chain(lines[last + 1..].iter())
            .copied()
            .collect(),
        _ => lines,
    };

    let result = kept.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        format!("{}\n", result)
    } else {
        result
    }
}

fn insert_validation_comments(content: &str, comments: &str) -> String {
    let mut lines: Vec<&str> = content.lines().collect();
    let insert_pos = if lines.first().is_some_and(|l| l.starts_with("#!")) {
        1
    } else {
        0
    };

    let comment_lines: Vec<&str> = comments.lines().collect();
    for (i, cl) in comment_lines.iter().enumerate() {
        lines.insert(insert_pos + i, cl);
    }

    let result = lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        format!("{}\n", result)
    } else {
        result
    }
}

/// Validate a script and display results.
pub fn handle_validate(validate_matches: &ArgMatches) {
    let raw_components: Vec<String> = validate_matches
        .get_many::<String>("command")
        .unwrap()
        .map(|s| s.to_string())
        .collect();

    let components: Vec<String> = raw_components
        .iter()
        .flat_map(|s| s.split('/'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if let Some(script_path) = find_script_file(&components) {
        let diagnostics = validate_script(&script_path);

        if has_errors(&diagnostics) {
            eprintln!("{}", format_diagnostics(&diagnostics));
            std::process::exit(1);
        }

        if !diagnostics.is_empty() {
            let warnings: Vec<_> = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Warning)
                .collect();
            if !warnings.is_empty() {
                eprintln!(
                    "{}",
                    warnings
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                eprintln!();
            }
        }

        let cmd_name = components.last().cloned().unwrap_or_default();
        let mut cmd = build_script_command_for_help(cmd_name, &script_path);
        println!("Script '{}' is valid.\n", script_path.display());
        cmd.print_help().unwrap();
        println!();
    } else {
        eprintln!("Script not found: {}", components.join("/"));
        std::process::exit(1);
    }
}

/// List scripts in the scripts directory (flat or tree).
pub fn handle_list(list_matches: &ArgMatches) {
    let subdir = list_matches
        .get_one::<String>("subdirectory")
        .map(|s| s.as_str());
    let tree = list_matches.get_flag("tree");
    let output = list_scripts(&get_scripts_dir(), subdir, tree);
    println!("{}", output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_validation_comments() {
        let content = "#!/bin/bash\n# ===========================================================\n# VALIDATION ERRORS — please fix and save to retry, or\n# close without saving to discard changes.\n# ===========================================================\n# error: duplicate argument name 'x'\n# ===========================================================\n#@description: my script\n";
        let stripped = strip_validation_comments(content);
        assert_eq!(stripped, "#!/bin/bash\n#@description: my script\n");
    }

    #[test]
    fn test_strip_no_validation_comments() {
        let content = "#!/bin/bash\n#@description: clean\n";
        let stripped = strip_validation_comments(content);
        assert_eq!(stripped, content);
    }

    #[test]
    fn test_insert_validation_comments_after_shebang() {
        let content = "#!/bin/bash\n#@description: my script\n";
        let comments = "# ===========================================================\n# error: bad\n# ===========================================================";
        let result = insert_validation_comments(content, comments);
        assert!(result.starts_with("#!/bin/bash\n# =========="));
        assert!(result.contains("#@description: my script"));
    }

    #[test]
    fn test_insert_validation_comments_no_shebang() {
        let content = "#@description: my script\n";
        let comments = "# ===========================================================\n# error: bad\n# ===========================================================";
        let result = insert_validation_comments(content, comments);
        assert!(result.starts_with("# =========="));
        assert!(result.contains("#@description: my script"));
    }

    #[test]
    fn test_strip_and_reinsert_roundtrip() {
        let original = "#!/bin/bash\n#@description: test\n#@arg:x - first\n";
        let comments = format_diagnostics_as_comments(&[crate::validation::ValidationDiagnostic {
            severity: crate::validation::Severity::Error,
            message: "test error".into(),
        }]);
        let annotated = insert_validation_comments(original, &comments);
        let stripped = strip_validation_comments(&annotated);
        assert_eq!(stripped, original);
    }
}
