use crate::metadata::{ArgType, CommandMetadata, LineType, parse_command_metadata};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ValidationDiagnostic {
    pub severity: Severity,
    pub message: String,
}

impl std::fmt::Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{}: {}", prefix, self.message)
    }
}

pub fn validate_script(path: &Path) -> Vec<ValidationDiagnostic> {
    let metadata = parse_command_metadata(path);
    validate_metadata(&metadata)
}

pub fn validate_metadata(metadata: &CommandMetadata) -> Vec<ValidationDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut found_catchall = false;
    let mut catchall_count = 0;

    for arg in &metadata.arguments {
        match arg {
            LineType::Positional(name, _desc, cfg) => {
                if name.is_empty() {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: "positional argument has an empty name".to_string(),
                    });
                }

                if !seen_names.insert(name.clone()) {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!("duplicate argument name '{}'", name),
                    });
                }

                if matches!(cfg.arg_type, Some(ArgType::CatchAll)) {
                    catchall_count += 1;
                    found_catchall = true;
                } else if found_catchall {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "positional argument '{}' appears after catch-all argument",
                            name
                        ),
                    });
                }

                if matches!(cfg.arg_type, Some(ArgType::Bool)) {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "positional argument '{}' cannot be 'bool' (only flags support bool)",
                            name
                        ),
                    });
                }

                if cfg.required && cfg.default.is_some() {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "argument '{}' has both 'required' and 'default' — 'required' will be ignored",
                            name
                        ),
                    });
                }

                if matches!(
                    cfg.arg_type,
                    Some(ArgType::File) | Some(ArgType::Dir) | Some(ArgType::Path)
                ) && !cfg.options.is_empty()
                {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "argument '{}' combines path type with 'options' — these are mutually exclusive",
                            name
                        ),
                    });
                }

                if matches!(cfg.arg_type, Some(ArgType::CatchAll)) && !cfg.options.is_empty() {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "catch-all argument '{}' has 'options' which won't be enforced per-value",
                            name
                        ),
                    });
                }
            }

            LineType::Flag(name, _desc, cfg) => {
                if name.is_empty() {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: "flag has an empty name".to_string(),
                    });
                }

                if !seen_names.insert(name.clone()) {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!("duplicate argument name '{}'", name),
                    });
                }

                if matches!(cfg.arg_type, Some(ArgType::Bool)) && !cfg.options.is_empty() {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "flag '{}' combines 'bool' with 'options' — these are mutually exclusive",
                            name
                        ),
                    });
                }

                if matches!(cfg.arg_type, Some(ArgType::Bool))
                    && matches!(
                        cfg.arg_type,
                        Some(ArgType::File) | Some(ArgType::Dir) | Some(ArgType::Path)
                    )
                {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "flag '{}' combines 'bool' with a path type — these are mutually exclusive",
                            name
                        ),
                    });
                }

                if matches!(
                    cfg.arg_type,
                    Some(ArgType::File) | Some(ArgType::Dir) | Some(ArgType::Path)
                ) && !cfg.options.is_empty()
                {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "flag '{}' combines path type with 'options' — these are mutually exclusive",
                            name
                        ),
                    });
                }

                if cfg.required && cfg.default.is_some() {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "flag '{}' has both 'required' and 'default' — 'required' will be ignored",
                            name
                        ),
                    });
                }

                if matches!(cfg.arg_type, Some(ArgType::CatchAll)) {
                    diagnostics.push(ValidationDiagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "flag '{}' cannot be a catch-all (only positional arguments support '...')",
                            name
                        ),
                    });
                }
            }

            LineType::Description(_) => {}
        }
    }

    if catchall_count > 1 {
        diagnostics.push(ValidationDiagnostic {
            severity: Severity::Error,
            message: "multiple catch-all arguments defined — only one is allowed".to_string(),
        });
    }

    diagnostics
}

pub fn has_errors(diagnostics: &[ValidationDiagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

pub fn format_diagnostics(diagnostics: &[ValidationDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats diagnostics as shell comments for embedding in a script file (kubectl-style).
pub fn format_diagnostics_as_comments(diagnostics: &[ValidationDiagnostic]) -> String {
    let mut lines = vec![
        "# ===========================================================".to_string(),
        "# VALIDATION ERRORS — please fix and save to retry, or".to_string(),
        "# close without saving to discard changes.".to_string(),
        "# ===========================================================".to_string(),
    ];
    for d in diagnostics {
        lines.push(format!("# {}", d));
    }
    lines.push("# ===========================================================".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Config;

    fn meta_with(args: Vec<LineType>) -> CommandMetadata {
        CommandMetadata {
            description: String::new(),
            arguments: args,
        }
    }

    #[test]
    fn test_valid_metadata_no_errors() {
        let m = meta_with(vec![
            LineType::Positional(
                "input".into(),
                "Input file".into(),
                Config {
                    required: true,
                    ..Default::default()
                },
            ),
            LineType::Flag(
                "verbose".into(),
                "Verbose".into(),
                Config {
                    arg_type: Some(ArgType::Bool),
                    ..Default::default()
                },
            ),
        ]);
        let d = validate_metadata(&m);
        assert!(d.is_empty(), "expected no diagnostics, got: {:?}", d);
    }

    #[test]
    fn test_duplicate_names() {
        let m = meta_with(vec![
            LineType::Positional("name".into(), "first".into(), Config::default()),
            LineType::Flag("name".into(), "second".into(), Config::default()),
        ]);
        let d = validate_metadata(&m);
        assert!(d.iter().any(|d| d.message.contains("duplicate")));
    }

    #[test]
    fn test_bool_on_positional() {
        let m = meta_with(vec![LineType::Positional(
            "flag-like".into(),
            "bad".into(),
            Config {
                arg_type: Some(ArgType::Bool),
                ..Default::default()
            },
        )]);
        let d = validate_metadata(&m);
        assert!(d.iter().any(|d| d.message.contains("cannot be 'bool'")));
    }

    #[test]
    fn test_required_and_default_warning() {
        let m = meta_with(vec![LineType::Flag(
            "f".into(),
            "desc".into(),
            Config {
                required: true,
                default: Some("val".into()),
                ..Default::default()
            },
        )]);
        let d = validate_metadata(&m);
        assert!(
            d.iter()
                .any(|d| d.severity == Severity::Warning && d.message.contains("required"))
        );
    }

    #[test]
    fn test_bool_with_options() {
        let m = meta_with(vec![LineType::Flag(
            "mode".into(),
            "desc".into(),
            Config {
                arg_type: Some(ArgType::Bool),
                options: vec!["a".into(), "b".into()],
                ..Default::default()
            },
        )]);
        let d = validate_metadata(&m);
        assert!(
            d.iter()
                .any(|d| d.message.contains("'bool' with 'options'"))
        );
    }

    #[test]
    fn test_path_with_options() {
        let m = meta_with(vec![LineType::Flag(
            "f".into(),
            "desc".into(),
            Config {
                arg_type: Some(ArgType::File),
                options: vec!["a".into()],
                ..Default::default()
            },
        )]);
        let d = validate_metadata(&m);
        assert!(
            d.iter()
                .any(|d| d.message.contains("path type with 'options'"))
        );
    }

    #[test]
    fn test_positional_after_catchall() {
        let m = meta_with(vec![
            LineType::Positional(
                "extra".into(),
                "catch-all".into(),
                Config {
                    arg_type: Some(ArgType::CatchAll),
                    ..Default::default()
                },
            ),
            LineType::Positional("late".into(), "after catchall".into(), Config::default()),
        ]);
        let d = validate_metadata(&m);
        assert!(d.iter().any(|d| d.message.contains("after catch-all")));
    }

    #[test]
    fn test_multiple_catchalls() {
        let m = meta_with(vec![
            LineType::Positional(
                "a".into(),
                "first".into(),
                Config {
                    arg_type: Some(ArgType::CatchAll),
                    ..Default::default()
                },
            ),
            LineType::Positional(
                "b".into(),
                "second".into(),
                Config {
                    arg_type: Some(ArgType::CatchAll),
                    ..Default::default()
                },
            ),
        ]);
        let d = validate_metadata(&m);
        assert!(d.iter().any(|d| d.message.contains("multiple catch-all")));
    }

    #[test]
    fn test_catchall_on_flag() {
        let m = meta_with(vec![LineType::Flag(
            "bad".into(),
            "desc".into(),
            Config {
                arg_type: Some(ArgType::CatchAll),
                ..Default::default()
            },
        )]);
        let d = validate_metadata(&m);
        assert!(
            d.iter()
                .any(|d| d.message.contains("cannot be a catch-all"))
        );
    }

    #[test]
    fn test_format_diagnostics_as_comments() {
        let diags = vec![ValidationDiagnostic {
            severity: Severity::Error,
            message: "something wrong".into(),
        }];
        let out = format_diagnostics_as_comments(&diags);
        assert!(out.contains("# error: something wrong"));
        assert!(out.contains("VALIDATION ERRORS"));
    }

    #[test]
    fn test_has_errors() {
        let only_warnings = vec![ValidationDiagnostic {
            severity: Severity::Warning,
            message: "warn".into(),
        }];
        assert!(!has_errors(&only_warnings));

        let with_error = vec![ValidationDiagnostic {
            severity: Severity::Error,
            message: "err".into(),
        }];
        assert!(has_errors(&with_error));
    }

    #[test]
    fn test_validate_script_from_file() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.sh");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            "#!/bin/bash\n#@description: Bad script\n#@arg:x - first\n#@arg:x - duplicate"
        )
        .unwrap();

        let d = validate_script(&path);
        assert!(d.iter().any(|d| d.message.contains("duplicate")));
    }
}
