use std::fs;
use std::path::{Path, PathBuf};

/// Metadata for a command parsed from its shell script
#[derive(Default)]
pub struct CommandMetadata {
    pub description: String,
    pub arguments: Vec<LineType>, // (name, description, required, default, options)
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Description(String),
    Flag(String, String, Config),
    Positional(String, String, Config),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ArgType {
    CatchAll,
    Bool,
    File,
    Dir,
    Path,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CompleteOptions {
    pub path: PathBuf,
    pub env_var: Option<String>,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Config {
    pub default: Option<String>,
    pub arg_type: Option<ArgType>,
    pub options: Vec<String>,
    pub complete_options: Option<CompleteOptions>,
    pub required: bool,
}

pub fn parse_command_metadata(path: &Path) -> CommandMetadata {
    let mut metadata = CommandMetadata::default();

    if let Ok(contents) = fs::read_to_string(path) {
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("#!") {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("#@") {
                if let Some(parsed) = parse_line(rest.trim()) {
                    match parsed {
                        LineType::Description(desc) => metadata.description = desc,
                        _ => metadata.arguments.push(parsed),
                    }
                }
            } else if trimmed.starts_with('#') {
                // Regular comment — skip but keep parsing
                continue;
            } else {
                // First non-comment line — stop parsing
                break;
            }
        }
    }

    metadata
}

fn parse_line(line: &str) -> Option<LineType> {
    if let Some(description) = line.strip_prefix("description:") {
        return Some(LineType::Description(description.trim().to_string()));
    }

    if let Some(flag) = line.strip_prefix("flag:")
        && let Some((clean_name, rest)) = flag.trim().split_once(" - ")
    {
        let (name, description, config) = parse_argument(clean_name, rest);
        return Some(LineType::Flag(name, description, config));
    }

    if let Some(arg) = line.strip_prefix("arg:")
        && let Some((clean_name, rest)) = arg.trim().split_once(" - ")
    {
        let (name, description, config) = parse_argument(clean_name, rest);
        return Some(LineType::Positional(name, description, config));
    }

    None
}

fn parse_argument(name: &str, rest: &str) -> (String, String, Config) {
    // extract annotations from the description
    // and remove them from the description
    // e.g. "description [dir,default:output.txt]" -> "description"
    if name == "..." {
        let cfg = Config {
            arg_type: Some(ArgType::CatchAll),
            ..Default::default()
        };
        return ("additional-args".to_string(), rest.trim().to_string(), cfg);
    }
    let (description, annotations) = extract_annotations(rest);
    let clean_description = description.trim().to_string();
    let config = parse_annotations(annotations);

    (
        name.to_string(),
        clean_description,
        config.unwrap_or_default(),
    )
}

fn parse_annotations(annotations: Vec<String>) -> Option<Config> {
    if annotations.is_empty() {
        return None;
    }

    let mut cfg = Config {
        default: None,
        arg_type: None,
        options: Vec::new(),
        complete_options: None,
        required: false,
    };

    for annotation in annotations {
        let (key, value) = split_once_or_all(annotation.trim(), ':');
        match key.trim() {
            "default" => cfg.default = Some(value.trim().to_string()),
            "required" => cfg.required = true,
            "bool" => cfg.arg_type = Some(ArgType::Bool),
            "dir" | "file" | "path" => {
                let arg_type = match key {
                    "dir" => ArgType::Dir,
                    "file" => ArgType::File,
                    "path" => ArgType::Path,
                    _ => unreachable!(),
                };
                cfg.arg_type = Some(arg_type);
                if !value.trim().is_empty() {
                    // Parse format: [file:default_path:ENV_VAR] or [file:default_path]
                    let parts: Vec<&str> = value.trim().splitn(2, ':').collect();
                    let path = PathBuf::from(parts[0].trim());
                    let env_var = parts.get(1).map(|s| s.trim().to_string());
                    cfg.complete_options = Some(CompleteOptions { path, env_var });
                }
            }
            "options" => {
                let options: Vec<String> = value.split('|').map(|s| s.trim().to_string()).collect();
                if let Some(default) = options
                    .iter()
                    .find(|s| s.starts_with('!') && s.ends_with('!'))
                {
                    cfg.default = Some(default.trim_matches('!').to_string());
                }

                cfg.options = options
                    .into_iter()
                    .map(|s| {
                        if s.starts_with('!') && s.ends_with('!') {
                            s.trim_matches('!').to_string()
                        } else {
                            s
                        }
                    })
                    .collect();
            }
            _ => {}
        }
    }

    // Warn if both required and default are set (contradictory)
    if cfg.required && cfg.default.is_some() {
        log::warn!(
            "Argument has both 'required' and 'default' set - 'required' will be ignored"
        );
        cfg.required = false;
    }

    Some(cfg)
}

fn extract_annotations(description: &str) -> (String, Vec<String>) {
    let mut annotations: Vec<String> = Vec::new();
    let mut desc = description.to_string();

    if let Some(start) = description.find('[')
        && let Some(end) = description[start..].find(']')
    {
        let a = description[start + 1..start + end].to_string();
        annotations = a.split(',').map(|s| s.trim().to_string()).collect();
        desc = description[..start].trim().to_string();
    }

    (desc, annotations)
}

fn split_once_or_all(s: &str, delim: char) -> (&str, &str) {
    match s.split_once(delim) {
        Some((left, right)) => (left, right),
        None => (s, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_test_script(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let script_path = dir.join(name);
        let mut file = File::create(&script_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        script_path
    }

    #[test]
    fn test_parse_command_metadata() {
        let script_content = r#"#!/bin/bash
#@description: Test script with various arguments and flags
#@arg:input - Input file path
#@arg:output - Output file path [dir,default:output.txt]
#@arg:... - Additional arguments
#@flag:verbose - Enable verbose output [bool]
#@flag:dry-run - Perform a dry run [default:false, bool]
#@flag:output-dir - Directory for output files [dir,required, default:./output]
#@flag:extra - Extra flag [default:opt1, options:opt1|opt2]
#@flag:debug - Enable debug mode [bool]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let metadata = parse_command_metadata(&script_path);

        // Test description
        assert_eq!(
            metadata.description,
            "Test script with various arguments and flags"
        );

        // Test arguments
        let input_arg = &metadata.arguments[0];
        assert_eq!(
            input_arg,
            &LineType::Positional(
                "input".to_string(),
                "Input file path".to_string(),
                Config::default()
            )
        );

        let output_arg = &metadata.arguments[1];
        assert_eq!(
            output_arg,
            &LineType::Positional(
                "output".to_string(),
                "Output file path".to_string(),
                Config {
                    default: Some("output.txt".to_string()),
                    arg_type: Some(ArgType::Dir),
                    ..Default::default()
                }
            )
        );

        // Test catch-all argument
        let catch_all_arg = &metadata.arguments[2];
        assert_eq!(
            catch_all_arg,
            &LineType::Positional(
                "additional-args".to_string(),
                "Additional arguments".to_string(),
                Config {
                    arg_type: Some(ArgType::CatchAll),
                    ..Default::default()
                }
            )
        );

        // Test verbose flag
        let verbose_flag = &metadata.arguments[3];
        assert_eq!(
            verbose_flag,
            &LineType::Flag(
                "verbose".to_string(),
                "Enable verbose output".to_string(),
                Config {
                    arg_type: Some(ArgType::Bool),
                    ..Default::default()
                }
            )
        );

        // Test dry-run flag
        let dry_run_flag = &metadata.arguments[4];
        assert_eq!(
            dry_run_flag,
            &LineType::Flag(
                "dry-run".to_string(),
                "Perform a dry run".to_string(),
                Config {
                    default: Some("false".to_string()),
                    arg_type: Some(ArgType::Bool),
                    ..Default::default()
                }
            )
        );

        // Test output-dir flag (required is ignored because default is set)
        let output_dir_flag = &metadata.arguments[5];
        assert_eq!(
            output_dir_flag,
            &LineType::Flag(
                "output-dir".to_string(),
                "Directory for output files".to_string(),
                Config {
                    default: Some("./output".to_string()),
                    arg_type: Some(ArgType::Dir),
                    required: false, // required is ignored when default is set
                    ..Default::default()
                }
            )
        );

        // Test extra flag
        let extra_flag = &metadata.arguments[6];
        assert_eq!(
            extra_flag,
            &LineType::Flag(
                "extra".to_string(),
                "Extra flag".to_string(),
                Config {
                    default: Some("opt1".to_string()),
                    options: vec!["opt1".to_string(), "opt2".to_string()],
                    ..Default::default()
                }
            )
        );

        let debug_flag = &metadata.arguments[7];
        assert_eq!(
            debug_flag,
            &LineType::Flag(
                "debug".to_string(),
                "Enable debug mode".to_string(),
                Config {
                    arg_type: Some(ArgType::Bool),
                    ..Default::default()
                }
            )
        );
    }

    #[test]
    fn test_required_with_default_ignored() {
        // When both required and default are set, required should be ignored
        let script_content = r#"#!/bin/bash
#@description: Test script
#@flag:test-flag - Test flag [required,default:value]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let metadata = parse_command_metadata(&script_path);

        let flag = &metadata.arguments[0];
        assert_eq!(
            flag,
            &LineType::Flag(
                "test-flag".to_string(),
                "Test flag".to_string(),
                Config {
                    default: Some("value".to_string()),
                    required: false, // should be false because default is set
                    ..Default::default()
                }
            )
        );
    }

    #[test]
    fn test_file_with_start_directory() {
        let script_content = r#"#!/bin/bash
#@description: Test script
#@flag:input - Input file [file:~/Documents]
#@arg:config - Config file [file:/etc]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let metadata = parse_command_metadata(&script_path);

        // Test flag with file and start directory
        let flag = &metadata.arguments[0];
        assert_eq!(
            flag,
            &LineType::Flag(
                "input".to_string(),
                "Input file".to_string(),
                Config {
                    arg_type: Some(ArgType::File),
                    complete_options: Some(CompleteOptions {
                        path: PathBuf::from("~/Documents"),
                        env_var: None,
                    }),
                    ..Default::default()
                }
            )
        );

        // Test arg with file and start directory
        let arg = &metadata.arguments[1];
        assert_eq!(
            arg,
            &LineType::Positional(
                "config".to_string(),
                "Config file".to_string(),
                Config {
                    arg_type: Some(ArgType::File),
                    complete_options: Some(CompleteOptions {
                        path: PathBuf::from("/etc"),
                        env_var: None,
                    }),
                    ..Default::default()
                }
            )
        );
    }

    #[test]
    fn test_file_with_env_var_override() {
        let script_content = r#"#!/bin/bash
#@description: Test script
#@flag:input - Input file [file:~/Documents:INPUT_DIR]
#@arg:config - Config file [dir:/etc:CONFIG_DIR]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let metadata = parse_command_metadata(&script_path);

        // Test flag with file, start directory, and env var
        let flag = &metadata.arguments[0];
        assert_eq!(
            flag,
            &LineType::Flag(
                "input".to_string(),
                "Input file".to_string(),
                Config {
                    arg_type: Some(ArgType::File),
                    complete_options: Some(CompleteOptions {
                        path: PathBuf::from("~/Documents"),
                        env_var: Some("INPUT_DIR".to_string()),
                    }),
                    ..Default::default()
                }
            )
        );

        // Test arg with dir, start directory, and env var
        let arg = &metadata.arguments[1];
        assert_eq!(
            arg,
            &LineType::Positional(
                "config".to_string(),
                "Config file".to_string(),
                Config {
                    arg_type: Some(ArgType::Dir),
                    complete_options: Some(CompleteOptions {
                        path: PathBuf::from("/etc"),
                        env_var: Some("CONFIG_DIR".to_string()),
                    }),
                    ..Default::default()
                }
            )
        );
    }

    #[test]
    fn test_parse_metadata_stops_at_code() {
        // Metadata after a non-comment line should be ignored
        let script_content = r#"#!/bin/bash
#@description: My tool
#@arg:input - Input file

echo "some code"

#@flag:verbose - Enable verbose output [bool]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let metadata = parse_command_metadata(&script_path);

        assert_eq!(metadata.description, "My tool");
        assert_eq!(metadata.arguments.len(), 1);
        assert_eq!(
            metadata.arguments[0],
            LineType::Positional(
                "input".to_string(),
                "Input file".to_string(),
                Config::default()
            )
        );
    }

    #[test]
    fn test_parse_metadata_skips_blank_lines_and_comments() {
        // Blank lines and regular comments within the header block should be skipped
        let script_content = r#"#!/bin/bash
#@description: My tool

# This is a regular comment
#@arg:input - Input file

#@flag:verbose - Enable verbose output [bool]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let metadata = parse_command_metadata(&script_path);

        assert_eq!(metadata.description, "My tool");
        assert_eq!(metadata.arguments.len(), 2);
        assert_eq!(
            metadata.arguments[0],
            LineType::Positional(
                "input".to_string(),
                "Input file".to_string(),
                Config::default()
            )
        );
        assert_eq!(
            metadata.arguments[1],
            LineType::Flag(
                "verbose".to_string(),
                "Enable verbose output".to_string(),
                Config {
                    arg_type: Some(ArgType::Bool),
                    ..Default::default()
                }
            )
        );
    }
}
