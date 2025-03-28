use std::fs;
use std::path::Path;

/// Metadata for a command parsed from its shell script
#[derive(Default)]
pub struct CommandMetadata {
    pub description: String,
    pub args: Vec<(String, String, Option<String>)>, // (name, description, default)
    pub flags: Vec<(String, String, bool, bool, Option<String>)>, // (name, description, required, is_bool, default)
    pub catch_all: Option<(String, String)>, // (name, description) for catching remaining arguments
}

/// Parses command metadata from a shell script
pub fn parse_command_metadata(path: &Path) -> CommandMetadata {
    let mut metadata = CommandMetadata::default();

    if let Ok(contents) = fs::read_to_string(path) {
        // Look for metadata in consecutive comment lines at the start of the file
        let lines: Vec<_> = contents.lines().collect();
        let mut i = 0;

        // Skip shebang if present
        if lines.first().is_some_and(|line| line.starts_with("#!")) {
            // if lines.first().map_or(false, |line| line.starts_with("#!")) {
            i += 1;
        }

        // Parse consecutive comment lines
        while i < lines.len() && lines[i].starts_with("#@") {
            let line = lines[i].trim_start_matches("#@").trim();

            // Parse description
            if line.starts_with("description:") {
                metadata.description = line.replace("description:", "").trim().to_string();
            }

            // Parse arguments
            if line.starts_with("arg:") {
                let arg_line = line.replace("arg:", "").trim().to_string();
                if let Some((name, desc)) = arg_line.split_once(" - ") {
                    let name = name.trim().to_string();
                    if name == "..." {
                        // This is a catch-all argument
                        metadata.catch_all = Some((name, desc.trim().to_string()));
                    } else {
                        let mut desc = desc.trim().to_string();
                        let default = if let Some(attrs_start) = desc.find('[') {
                            if let Some(attrs_end) = desc[attrs_start..].find(']') {
                                let attrs = desc[attrs_start + 1..attrs_start + attrs_end].trim();
                                let mut default_value = None;

                                // Parse attributes
                                for attr in attrs.split(',') {
                                    let attr = attr.trim();
                                    if let Some((key, value)) = attr.split_once(':') {
                                        if key.trim() == "default" {
                                            default_value = Some(value.trim().to_string());
                                        }
                                    }
                                }

                                // Remove the attributes from description
                                desc = desc[..attrs_start].trim().to_string();
                                default_value
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        metadata.args.push((name, desc, default));
                    }
                }
            }

            // Parse flags
            if line.starts_with("flag:") || line.starts_with("bool:") {
                let is_bool = line.starts_with("bool:");
                let flag_line = line
                    .replace(if is_bool { "bool:" } else { "flag:" }, "")
                    .trim()
                    .to_string();
                if let Some((name, desc)) = flag_line.split_once(" - ") {
                    let name = name.trim().to_string();
                    let mut desc = desc.trim().to_string();
                    let mut required = false;
                    let mut default = None;

                    // Parse attributes from brackets
                    if let Some(attrs_start) = desc.find('[') {
                        if let Some(attrs_end) = desc[attrs_start..].find(']') {
                            let attrs = desc[attrs_start + 1..attrs_start + attrs_end].trim();

                            // Parse attributes
                            for attr in attrs.split(',') {
                                let attr = attr.trim();
                                if attr == "required" {
                                    required = true;
                                } else if let Some((key, value)) = attr.split_once(':') {
                                    if key.trim() == "default" {
                                        default = Some(value.trim().to_string());
                                    }
                                }
                            }

                            // Remove the attributes from description
                            desc = desc[..attrs_start].trim().to_string();
                        }
                    }

                    metadata
                        .flags
                        .push((name, desc, required, is_bool, default));
                }
            }

            i += 1;
        }
    }

    metadata
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
#@bool:dry-run - Perform a dry run [default:false]
#@flag:output-dir - Directory for output files [required, default:./output]
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
        let (input_name, input_desc, input_default) = &metadata.args[0];
        assert_eq!(input_name, "input");
        assert_eq!(input_desc, "Input file path");
        assert!(input_default.is_none());

        let (output_name, output_desc, output_default) = &metadata.args[1];
        assert_eq!(output_name, "output");
        assert_eq!(output_desc, "Output file path");
        assert_eq!(output_default.as_deref(), Some("output.txt"));

        // Test catch-all argument
        assert!(metadata.catch_all.is_some());
        let (catch_all_name, catch_all_desc) = metadata.catch_all.unwrap();
        assert_eq!(catch_all_name, "...");
        assert_eq!(catch_all_desc, "Additional arguments");

        // Test flags
        assert_eq!(metadata.flags.len(), 3);

        // Test verbose flag
        let (verbose_name, verbose_desc, verbose_required, verbose_is_bool, verbose_default) =
            &metadata.flags[0];
        assert_eq!(verbose_name, "verbose");
        assert_eq!(verbose_desc, "Enable verbose output");
        assert!(verbose_required);
        assert!(!verbose_is_bool);
        assert!(verbose_default.is_none());

        // Test dry-run flag
        let (dry_run_name, dry_run_desc, dry_run_required, dry_run_is_bool, dry_run_default) =
            &metadata.flags[1];
        assert_eq!(dry_run_name, "dry-run");
        assert_eq!(dry_run_desc, "Perform a dry run");
        assert!(!dry_run_required);
        assert!(dry_run_is_bool);
        assert_eq!(dry_run_default.as_deref(), Some("false"));

        // Test output-dir flag
        let (
            output_dir_name,
            output_dir_desc,
            output_dir_required,
            output_dir_is_bool,
            output_dir_default,
        ) = &metadata.flags[2];
        assert_eq!(output_dir_name, "output-dir");
        assert_eq!(output_dir_desc, "Directory for output files");
        assert!(output_dir_required);
        assert!(!output_dir_is_bool);
        assert_eq!(output_dir_default.as_deref(), Some("./output"));
    }
}
