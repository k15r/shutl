use std::fs;
use std::path::Path;

/// Metadata for a command parsed from its shell script
#[derive(Default)]
pub struct CommandMetadata {
    pub description: String,
    pub args: Vec<Arg>,                      // (name, description, default)
    pub flags: Vec<Flag>, // (name, description, required, is_bool, default, options)
    pub catch_all: Option<(String, String)>, // (name, description) for catching remaining arguments
}

pub struct Arg {
    pub name: String,
    pub description: String,
    pub default: Option<String>,
}

pub struct Flag {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub is_bool: bool,
    pub default: Option<String>,
    pub options: Vec<String>,
}

pub fn parse_command_metadata(path: &Path) -> CommandMetadata {
    let mut metadata = CommandMetadata::default();
    let comment_prefix = match path.extension().and_then(|ext| ext.to_str()) {
        Some("js") => "//@",
        _ => "#@",
    };

    if let Ok(contents) = fs::read_to_string(path) {
        let lines = contents.lines().collect::<Vec<_>>();
        let mut i = if lines.first().is_some_and(|line| line.starts_with("#!")) {
            1
        } else {
            0
        };

        while i < lines.len() && lines[i].starts_with(comment_prefix) {
            let line = lines[i].trim_start_matches(comment_prefix).trim();
            if line.starts_with("description:") {
                metadata.description = line
                    .strip_prefix("description:")
                    .unwrap()
                    .trim()
                    .to_string();
            } else if line.starts_with("arg:") {
                parse_arg(&mut metadata, line);
            } else if line.starts_with("flag:") {
                parse_flag(&mut metadata, line);
            }
            i += 1;
        }
    }

    metadata
}

fn parse_flag(metadata: &mut CommandMetadata, line: &str) {
    let (name, description) = line["flag:".len()..]
        .trim()
        .split_once(" - ")
        .unwrap_or_default();
    let mut flag = Flag {
        name: name.trim().to_string(),
        description: description.to_string(),
        required: false,
        is_bool: false,
        default: None,
        options: Vec::new(),
    };

    if let Some(attrs_start) = description.find('[') {
        if let Some(attrs_end) = description[attrs_start..].find(']') {
            let attrs = &description[attrs_start + 1..attrs_start + attrs_end];
            for attr in attrs.split(',') {
                match attr.trim() {
                    "bool" => flag.is_bool = true,
                    "required" => flag.required = true,
                    attr => {
                        if let Some((key, value)) = attr.split_once(':') {
                            match key.trim() {
                                "default" => flag.default = Some(value.trim().to_string()),
                                "options" => {
                                    flag.options =
                                        value.split('|').map(|s| s.trim().to_string()).collect()
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            flag.description = description[..attrs_start].trim().to_string();
        }
    }

    metadata.flags.push(flag);
}

fn parse_arg(metadata: &mut CommandMetadata, line: &str) {
    let (name, description) = line["arg:".len()..]
        .trim()
        .split_once(" - ")
        .unwrap_or_default();
    if name == "..." {
        metadata.catch_all = Some((name.to_string(), description.trim().to_string()));
    } else {
        let mut arg = Arg {
            name: name.trim().to_string(),
            description: description.trim().to_string(),
            default: None,
        };

        if let Some(attrs_start) = description.find('[') {
            if let Some(attrs_end) = description[attrs_start..].find(']') {
                let attrs = &description[attrs_start + 1..attrs_start + attrs_end];
                for attr in attrs.split(',') {
                    if let Some((key, value)) = attr.trim().split_once(':') {
                        if key.trim() == "default" {
                            arg.default = Some(value.trim().to_string());
                        }
                    }
                }
                arg.description = description[..attrs_start].trim().to_string();
            }
        }
        metadata.args.push(arg);
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
#@arg:output - Output file path [default:output.txt]
#@arg:... - Additional arguments
#@flag:verbose - Enable verbose output [required]
#@flag:dry-run - Perform a dry run [default:false, bool]
#@flag:output-dir - Directory for output files [required, default:./output]
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
        assert_eq!(metadata.args.len(), 2);
        let input_arg = &metadata.args[0];
        assert_eq!(input_arg.name, "input");
        assert_eq!(input_arg.description, "Input file path");
        assert!(input_arg.default.is_none());

        let output_arg = &metadata.args[1];
        assert_eq!(output_arg.name, "output");
        assert_eq!(output_arg.description, "Output file path");
        assert_eq!(output_arg.default.as_deref(), Some("output.txt"));

        // Test catch-all argument
        assert!(metadata.catch_all.is_some());
        let (catch_all_name, catch_all_desc) = metadata.catch_all.unwrap();
        assert_eq!(catch_all_name, "...");
        assert_eq!(catch_all_desc, "Additional arguments");

        // Test flags
        assert_eq!(metadata.flags.len(), 5);

        // Test verbose flag
        let verbose_flag = &metadata.flags[0];
        assert_eq!(verbose_flag.name, "verbose");
        assert_eq!(verbose_flag.description, "Enable verbose output");
        assert!(verbose_flag.required);
        assert!(!verbose_flag.is_bool);
        assert!(verbose_flag.default.is_none());

        // Test dry-run flag
        let dry_run_flag = &metadata.flags[1];
        assert_eq!(dry_run_flag.name, "dry-run");
        assert_eq!(dry_run_flag.description, "Perform a dry run");
        assert!(!dry_run_flag.required);
        assert!(dry_run_flag.is_bool);
        assert_eq!(dry_run_flag.default.as_deref(), Some("false"));

        // Test output-dir flag
        let output_dir_flag = &metadata.flags[2];
        assert_eq!(output_dir_flag.name, "output-dir");
        assert_eq!(output_dir_flag.description, "Directory for output files");
        assert!(output_dir_flag.required);
        assert!(!output_dir_flag.is_bool);
        assert_eq!(output_dir_flag.default.as_deref(), Some("./output"));

        // Test extra flag
        let extra_flag = &metadata.flags[3];
        assert_eq!(extra_flag.name, "extra");
        assert_eq!(extra_flag.description, "Extra flag");
        assert!(!extra_flag.required);
        assert!(!extra_flag.is_bool);
        assert_eq!(extra_flag.default.as_deref(), Some("opt1"));
        assert_eq!(
            &extra_flag.options,
            &vec!["opt1".to_string(), "opt2".to_string()]
        );

        let debug_flag = &metadata.flags[4];
        assert_eq!(debug_flag.name, "debug");
        assert_eq!(debug_flag.description, "Enable debug mode");
        assert!(!debug_flag.required);
        assert!(debug_flag.is_bool);
        assert!(debug_flag.default.is_none());
        assert!(debug_flag.options.is_empty());
    }
}
