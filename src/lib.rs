use dirs::home_dir;
use std::path::PathBuf;

pub mod command;
pub mod metadata;
pub mod script;

pub use command::build_cli_command;
pub use metadata::CommandMetadata;
pub use script::{execute_script, find_script_file};

/// The directory name where scripts are stored
const SCRIPTS_DIR_NAME: &str = ".shutl";

/// Gets the path to the scripts directory
pub fn get_scripts_dir() -> PathBuf {
    // check if SHUTL_DIR is set
    if let Ok(shutl_dir) = std::env::var("SHUTL_DIR") {
        // Expand ~ and env vars in the path
        if let Ok(expanded) = shellexpand::full(&shutl_dir) {
            return PathBuf::from(expanded.to_string());
        }
        return PathBuf::from(shutl_dir);
    }
    let mut path = home_dir().expect("Could not determine home directory");
    path.push(SCRIPTS_DIR_NAME);

    // Create the directory if it doesn't exist
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create scripts directory");
    }

    path
}

/// Resolves the editor to use, checking the provided override, then $EDITOR, then defaulting to vim
pub fn resolve_editor(editor_override: Option<&String>) -> String {
    editor_override
        .cloned()
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| "vim".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_editor_with_override() {
        let editor = String::from("nano");
        assert_eq!(resolve_editor(Some(&editor)), "nano");
    }

    #[test]
    fn test_resolve_editor_default() {
        // Clear EDITOR env var for this test
        unsafe { std::env::remove_var("EDITOR") };
        assert_eq!(resolve_editor(None), "vim");
    }

    #[test]
    fn test_resolve_editor_from_env() {
        unsafe { std::env::set_var("EDITOR", "emacs") };
        assert_eq!(resolve_editor(None), "emacs");
        unsafe { std::env::remove_var("EDITOR") };
    }
}
