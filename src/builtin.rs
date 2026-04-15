//! Built-in subcommands: new, edit, list.

use clap::ArgMatches;
use std::process::Command;

use crate::command::list_scripts;
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

/// Edit an existing script by path components.
pub fn handle_edit(edit_matches: &ArgMatches) {
    let raw_components: Vec<String> = edit_matches
        .get_many::<String>("command")
        .unwrap()
        .map(|s| s.to_string())
        .collect();

    // Flatten components. split any path separators into separate components
    let components: Vec<String> = raw_components
        .iter()
        .flat_map(|s| s.split('/'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let editor = edit_matches.get_one::<String>("editor");

    // Find the script file in the command structure
    if let Some(script_path) = find_script_file(&components) {
        let editor = resolve_editor(editor);

        Command::new(&editor)
            .arg(&script_path)
            .status()
            .expect("Failed to open editor");

        println!("Edited script: {}", script_path.display());
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
