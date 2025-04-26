use crate::get_scripts_dir;
use crate::metadata::parse_command_metadata;
use clap::ArgMatches;
use is_executable::IsExecutable;
use std::path::Path;
use std::process::Command as ProcessCommand;

/// Executes a script with the provided arguments
pub fn execute_script(script_path: &Path, matches: &ArgMatches) -> std::io::Result<()> {
    // Determine the interpreter based on file extension
    let interpreter = match script_path.extension().and_then(|ext| ext.to_str()) {
        Some("sh") => "bash",
        Some("py") => "python3",
        Some("rb") => "ruby",
        Some("js") => "node",
        _ => "bash", // Default to bash for files without extension
    };

    let mut command = ProcessCommand::new(interpreter);
    command.arg(script_path);

    // Check if the script is executable
    // if it is executable use it directly
    if script_path.is_executable() {
        command = ProcessCommand::new(script_path);
    }

    // Build the command with the appropriate interpreter

    let metadata = parse_command_metadata(script_path);

    // Add positional arguments as environment variables
    for (arg_name, _, default) in metadata.args {
        let env_name = format!("CLI_{}", arg_name.replace('-', "_").to_uppercase());
        let value = if let Some(value) = matches.get_one::<String>(&arg_name) {
            value.as_str()
        } else if let Some(ref default_value) = default {
            default_value.as_str()
        } else {
            ""
        };
        command.env(&env_name, value);
    }

    // Add flags as environment variables
    for (flag_name, _, _, is_bool, default, options) in metadata.flags {
        let env_name = format!("CLI_{}", flag_name.replace('-', "_").to_uppercase());
        let value = if is_bool {
            // For boolean flags:
            // - If --no-flag is specified, set to false
            // - If --flag is specified, set to true
            // - If neither is specified, use default value
            let negated_name = format!("no-{}", flag_name);
            if matches.get_flag(&negated_name) {
                "false"
            } else if matches.get_flag(&flag_name) {
                "true"
            } else if let Some(ref default_value) = default {
                default_value.as_str()
            } else {
                "false"
            }
        } else {
            // For non-boolean flags, use the provided value or default
            if matches.contains_id(&flag_name) {
                matches
                    .get_one::<String>(&flag_name)
                    .map(|s| s.as_str())
                    .unwrap_or("")
            } else if let Some(ref default_value) = default {
                default_value.as_str()
            } else {
                ""
            }
        };
        command.env(&env_name, value);
    }

    // Add catch-all arguments as environment variable
    if let Some((_, _)) = metadata.catch_all {
        if let Some(values) = matches.get_many::<String>("additional-args") {
            let values_vec: Vec<_> = values.collect();
            let env_value = values_vec
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            command.env("CLI_ADDITIONAL_ARGS", env_value);
        }
    }

    // Execute the command
    let status = command.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Recursively finds a script file in the scripts directory
pub fn find_script_file(components: &[String]) -> Option<std::path::PathBuf> {
    find_script_file_in_dir(components, &get_scripts_dir())
}

/// Recursively finds a script file in the specified directory
pub fn find_script_file_in_dir(
    components: &[String],
    base_dir: &Path,
) -> Option<std::path::PathBuf> {
    let mut path = base_dir.to_path_buf();

    // Add all components except the last one as directories
    for component in components.iter().take(components.len() - 1) {
        path.push(component);
    }

    // Add the last component as a file
    let last_component = components.last().unwrap();
    path.push(last_component);

    // First try exact match
    if path.exists() {
        return Some(path);
    }

    // If not found, try with common script extensions
    for ext in ["sh", "py", "rb", "js"] {
        path.set_extension(ext);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_test_script(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let script_path = dir.join(name);
        // Create parent directories if they don't exist
        if let Some(parent) = script_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = File::create(&script_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        script_path
    }

    #[test]
    fn test_find_script_file() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        std::fs::create_dir(&scripts_dir).unwrap();

        // Create test scripts with different extensions
        create_test_script(&scripts_dir, "test1.sh", "#!/bin/bash");
        create_test_script(&scripts_dir, "test2.py", "#!/usr/bin/env python3");
        create_test_script(&scripts_dir, "subdir/test3.rb", "#!/usr/bin/env ruby");

        // Test finding script with .sh extension
        let components = vec!["test1".to_string()];
        let script_path = find_script_file_in_dir(&components, &scripts_dir).unwrap();
        assert_eq!(script_path.file_name().unwrap(), "test1.sh");

        // Test finding script with .py extension
        let components = vec!["test2".to_string()];
        let script_path = find_script_file_in_dir(&components, &scripts_dir).unwrap();
        assert_eq!(script_path.file_name().unwrap(), "test2.py");

        // Test finding script with .rb extension
        let components = vec!["subdir".to_string(), "test3".to_string()];
        let script_path = find_script_file_in_dir(&components, &scripts_dir).unwrap();
        assert_eq!(script_path.file_name().unwrap(), "test3.rb");

        // Test non-existent script
        let components = vec!["nonexistent".to_string()];
        assert!(find_script_file_in_dir(&components, &scripts_dir).is_none());
    }

    #[test]
    fn test_execute_script_with_different_extensions() {
        let dir = tempdir().unwrap();

        // Create test scripts with different extensions
        let sh_script = create_test_script(
            &dir.path(),
            "test.sh",
            r#"#!/bin/bash
#@description: Test shell script
#@arg:input - Input file
echo "Shell script executed with input: $CLI_INPUT"
"#,
        );

        let py_script = create_test_script(
            &dir.path(),
            "test.py",
            r#"#!/usr/bin/env python3
import os
#@description: Test Python script
#@arg:input - Input file
print(f"Python script executed with input: {os.environ.get('CLI_INPUT', '')}")
"#,
        );

        let rb_script = create_test_script(
            &dir.path(),
            "test.rb",
            r#"#!/usr/bin/env ruby
#@description: Test Ruby script
#@arg:input - Input file
puts "Ruby script executed with input: #{ENV['CLI_INPUT']}"
"#,
        );

        // Create test matches
        let matches = clap::Command::new("test")
            .arg(clap::Arg::new("input").required(true))
            .get_matches_from(vec!["test", "test.txt"]);

        // Test shell script execution
        assert!(execute_script(&sh_script, &matches).is_ok());

        // Test Python script execution
        assert!(execute_script(&py_script, &matches).is_ok());

        // Test Ruby script execution
        assert!(execute_script(&rb_script, &matches).is_ok());
    }
}
