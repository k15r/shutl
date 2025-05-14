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

// For backward compatibility
pub const SCRIPTS_DIR: &str = SCRIPTS_DIR_NAME;
