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
    pub extensions: Vec<String>,
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
            let parsed_line = parse_line(line);
            if let Some(parsed) = parsed_line {
                match parsed {
                    LineType::Description(desc) => metadata.description = desc,
                    _ => {
                        metadata.arguments.push(parsed);
                    }
                }
            }
            i += 1;
        }
    }

    metadata
}

fn parse_line(line: &str) -> Option<LineType> {
    if let Some(description) = line.strip_prefix("description:") {
        return Some(LineType::Description(description.trim().to_string()));
    }

    if let Some(flag) = line.strip_prefix("flag:") {
        if let Some((clean_name, rest)) = flag.trim().split_once(" - ") {
            let (name, description, config) = parse_argument(clean_name, rest);
            return Some(LineType::Flag(name, description, config));
        }
    }

    if let Some(arg) = line.strip_prefix("arg:") {
        if let Some((clean_name, rest)) = arg.trim().split_once(" - ") {
            let (name, description, config) = parse_argument(clean_name, rest);
            return Some(LineType::Positional(name, description, config));
        }
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
        return (
            "additional-arguments".to_string(),
            rest.trim().to_string(),
            cfg,
        );
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
                    cfg.complete_options = Some(CompleteOptions {
                        path: PathBuf::from(value.trim()),
                        extensions: Vec::new(),
                    });
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
    Some(cfg)
}

fn extract_annotations(description: &str) -> (String, Vec<String>) {
    let mut annotations: Vec<String> = Vec::new();
    let mut desc = description.to_string();

    if let Some(start) = description.find('[') {
        if let Some(end) = description[start..].find(']') {
            let a = description[start + 1..start+end].to_string();
            annotations = a.split(',').map(|s| s.trim().to_string()).collect();
            desc = description[..start].trim().to_string();
        }
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
        assert_eq!(input_arg, &LineType::Positional(
            "input".to_string(),
            "Input file path".to_string(),
            Config::default()
        ));

        let output_arg = &metadata.arguments[1];
        assert_eq!(output_arg, &LineType::Positional(
            "output".to_string(),
            "Output file path".to_string(),
            Config {
                default: Some("output.txt".to_string()),
                arg_type: Some(ArgType::Dir),
                ..Default::default()
            }
        ));

        // Test catch-all argument
        let catch_all_arg = &metadata.arguments[2];
        assert_eq!(catch_all_arg, &LineType::Positional(
            "additional-arguments".to_string(),
            "Additional arguments".to_string(),
            Config {
                arg_type: Some(ArgType::CatchAll),
                ..Default::default()
            }
        ));
        
        // Test verbose flag
        let verbose_flag = &metadata.arguments[3];
        assert_eq!(verbose_flag, &LineType::Flag(
            "verbose".to_string(),
            "Enable verbose output".to_string(),
            Config {
                arg_type: Some(ArgType::Bool),
                ..Default::default()
            }
        ));

        // Test dry-run flag
        let dry_run_flag = &metadata.arguments[4];
        assert_eq!(dry_run_flag, &LineType::Flag(
            "dry-run".to_string(),
            "Perform a dry run".to_string(),
            Config {
                default: Some("false".to_string()),
                arg_type: Some(ArgType::Bool),
                ..Default::default()
            }
        ));

        // Test output-dir flag
        let output_dir_flag = &metadata.arguments[5];
        assert_eq!(output_dir_flag, &LineType::Flag(
            "output-dir".to_string(),
            "Directory for output files".to_string(),
            Config {
                default: Some("./output".to_string()),
                arg_type: Some(ArgType::Dir),
                required: true,
                ..Default::default()
            }
        ));

        // Test extra flag
        let extra_flag = &metadata.arguments[6];
        assert_eq!(extra_flag, &LineType::Flag(
            "extra".to_string(),
            "Extra flag".to_string(),
            Config {
                default: Some("opt1".to_string()),
                options: vec!["opt1".to_string(), "opt2".to_string()],
                ..Default::default()
            }
        ));

        let debug_flag = &metadata.arguments[7];
        assert_eq!(debug_flag, &LineType::Flag(
            "debug".to_string(),
            "Enable debug mode".to_string(),
            Config {
                arg_type: Some(ArgType::Bool),
                ..Default::default()
            }
        ));
    }
}
