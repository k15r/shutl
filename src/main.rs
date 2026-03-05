use clap::ArgMatches;
use shutl::{build_cli_command, execute_script, find_script_file, get_scripts_dir, resolve_editor};
use shutl::command::list_scripts;
use std::process::Command;

fn main() {
    env_logger::builder().init();

    log::debug!("args: {:?}", std::env::args().collect::<Vec<_>>());

    clap_complete::CompleteEnv::with_factory(build_cli_command).complete();

    let cli = build_cli_command();
    let mut cli_for_help = cli.clone();
    let matches = cli.get_matches();

    match matches.subcommand() {
        Some(("new", sub_matches)) => handle_new(sub_matches),
        Some(("edit", sub_matches)) => handle_edit(sub_matches),
        Some(("list", sub_matches)) => handle_list(sub_matches),
        Some((command, sub_matches)) => execute_command(command, sub_matches),
        None => {
            cli_for_help.print_help().unwrap();
            std::process::exit(1);
        }
    }
}

fn execute_command(command: &str, sub_m: &ArgMatches) {
    // Collect all command components
    let mut components = vec![command.to_string()];
    let mut current = sub_m;
    while let Some((subcommand, sub_matches)) = current.subcommand() {
        components.push(subcommand.to_string());
        current = sub_matches;
    }

    // Check if this is a directory command
    let mut path = get_scripts_dir();
    for component in &components {
        path.push(component);
    }

    if path.is_dir() {
        // Build a new command tree starting from this directory
        let mut dir_cli = clap::Command::new(components.join(" ")).disable_help_subcommand(true);
        for cmd_with_path in shutl::command::build_command_tree(&path, &components) {
            dir_cli = dir_cli.subcommand(cmd_with_path.command);
        }
        // Show help for this directory command
        dir_cli.print_help().unwrap();
        std::process::exit(1);
    }

    // Find the script file in the original directory structure
    if let Some(script_path) = find_script_file(&components) {
        // Execute the script with the arguments
        if let Err(e) = execute_script(&script_path, current) {
            eprintln!("Error executing command: {}", e);
            std::process::exit(1);
        }
    } else {
        eprintln!("Script not found: {}", components.join("/"));
        std::process::exit(1);
    }
}

fn handle_list(list_matches: &ArgMatches) {
    let subdir = list_matches.get_one::<String>("subdirectory").map(|s| s.as_str());
    let tree = list_matches.get_flag("tree");
    let output = list_scripts(&get_scripts_dir(), subdir, tree);
    println!("{}", output);
}

fn handle_edit(edit_matches: &ArgMatches) {
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

fn handle_new(new_matches: &ArgMatches) {
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
    if let Some(parent) = script_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Failed to create directory {}: {}", parent.display(), e);
            std::process::exit(1);
        }
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
        eprintln!("Failed to set permissions on {}: {}", script_path.display(), e);
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
