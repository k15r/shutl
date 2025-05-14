use crate::get_scripts_dir;
use crate::metadata::{ArgType, LineType, parse_command_metadata};
use clap::ArgMatches;
use log::debug;
use std::path::Path;
use std::process::Command as ProcessCommand;

/// Executes a script with the provided arguments
pub fn execute_script(script_path: &Path, matches: &ArgMatches) -> std::io::Result<()> {
    let mut command = ProcessCommand::new(script_path);
    let metadata = parse_command_metadata(script_path);

    for arg in metadata.arguments {
        match arg {
            LineType::Positional(name, _, config) => {
                if let Some(ArgType::CatchAll) = config.arg_type {
                    debug!("catch-all: {}", name);
                    if let Some(values) = matches.get_many::<String>("additional-args") {
                        debug!("additional-args: {:?}", values);
                        let env_value = values.map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
                        debug!("additional-args: {:?}", env_value);
                        command.env("SHUTL_ADDITIONAL_ARGS", env_value);
                    }
                } else {
                    let env_name = format!("SHUTL_{}", name.replace('-', "_").to_uppercase());
                    let value = matches
                        .get_one::<String>(name.as_str())
                        .map(|v| v.as_str())
                        .unwrap_or_else(|| config.default.as_deref().unwrap_or(""));
                    command.env(&env_name, value);
                }
            }
            LineType::Flag(name, _, config) => {
                let env_name = format!("SHUTL_{}", name.replace('-', "_").to_uppercase());
                let value = if config.arg_type == Some(ArgType::Bool) {
                    let negated_name = format!("no-{}", name);
                    if matches.get_flag(&negated_name) {
                        "false"
                    } else if matches.get_flag(name.as_str()) {
                        "true"
                    } else {
                        config.default.as_deref().unwrap_or("false")
                    }
                } else {
                    matches
                        .get_one::<String>(name.as_str())
                        .map(|v| v.as_str())
                        .unwrap_or_else(|| config.default.as_deref().unwrap_or(""))
                };
                command.env(&env_name, value);
            }
            _ => {}
        }
    }

    if matches.get_flag("shutlverboseid") {
        println!("Environment variables:");
        for (key, value) in command.get_envs() {
            println!(
                "{}: {}",
                key.to_str().unwrap(),
                value.unwrap().to_str().unwrap()
            );
        }

        println!("Command: {:?}", command.get_program());
    }

    // debug the command env
    debug!("Command Envs: {:?}", command.get_envs());
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

pub fn find_script_file_in_dir(
    components: &[String],
    base_dir: &Path,
) -> Option<std::path::PathBuf> {
    let mut path = base_dir.to_path_buf();

    // Build the path using all components except the last one
    for component in &components[..components.len() - 1] {
        path.push(component);
    }
    path.push(components.last()?);

    // Check for an exact match
    if path.exists() {
        return Some(path);
    }

    // Check for files starting with the last component in the parent directory
    path.pop();
    std::fs::read_dir(&path)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            if entry.path().is_dir() {
                return None;
            }
            entry
                .file_name()
                .to_str()?
                .starts_with(components.last()?)
                .then_some(entry.path())
        })
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
        // Make the script executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata().unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script_path, perms).unwrap();
        }

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
echo "Shell script executed with input: $SHUTL_INPUT"
"#,
        );

        let py_script = create_test_script(
            &dir.path(),
            "test.py",
            r#"#!/usr/bin/env python3
import os
#@description: Test Python script
#@arg:input - Input file
print(f"Python script executed with input: {os.environ.get('SHUTL_INPUT', '')}")
"#,
        );

        let rb_script = create_test_script(
            &dir.path(),
            "test.rb",
            r#"#!/usr/bin/env ruby
#@description: Test Ruby script
#@arg:input - Input file
puts "Ruby script executed with input: #{ENV['SHUTL_INPUT']}"
"#,
        );

        // Create test matches
        let matches = clap::Command::new("test")
            .arg(
                clap::Arg::new("shutlverboseid")
                    .long("shutl-verbose")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(clap::Arg::new("input").required(true))
            .get_matches_from(vec!["test", "test.txt", "--shutl-verbose"]);

        // Test shell script execution
        assert!(execute_script(&sh_script, &matches).is_ok());

        // Test Python script execution
        assert!(execute_script(&py_script, &matches).is_ok());

        // Test Ruby script execution
        assert!(execute_script(&rb_script, &matches).is_ok());
    }
}
