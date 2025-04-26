use shutl::{build_cli_command, execute_script, find_script_file, get_scripts_dir};
use std::env;
use std::process::Command;

fn main() {
    clap_complete::CompleteEnv::with_factory(build_cli_command).complete();
    // Build the CLI command structure
    let mut cli = build_cli_command();
    let mut cli_for_help = cli.clone();

    // Add the new command
    cli = cli.subcommand(
        clap::Command::new("new")
            .about("Create a new script")
            .arg(
                clap::Arg::new("location")
                    .help("Location to create the script (relative to ~/.shutl)")
                    .default_value("")
                    .required(true),
            )
            .arg(
                clap::Arg::new("name")
                    .help("Name of the script (without .sh extension)")
                    .required(true),
            )
            .arg(
                clap::Arg::new("editor")
                    .help("Editor to use (defaults to $EDITOR or 'vim')")
                    .long("editor")
                    .short('e'),
            )
            .arg(
                clap::Arg::new("no-edit")
                    .help("Don't open the script in an editor")
                    .long("no-edit")
                    .action(clap::ArgAction::SetTrue),
            ),
    );

    // Add the edit command
    cli = cli.subcommand(
        clap::Command::new("edit")
            .about("Edit an existing script")
            .arg(
                clap::Arg::new("command")
                    .help("Command path components (e.g., 'subdir myscript')")
                    .required(true)
                    .num_args(1..),
            )
            .arg(
                clap::Arg::new("editor")
                    .help("Editor to use (defaults to $EDITOR or 'vim')")
                    .long("editor")
                    .short('e'),
            ),
    );

    // Get matches for command processing
    let matches = cli.get_matches();

    // Handle the new command
    if let Some(("new", new_matches)) = matches.subcommand() {
        let name = new_matches.get_one::<String>("name").unwrap();
        let location = new_matches.get_one::<String>("location").unwrap();
        let editor = new_matches.get_one::<String>("editor");
        let no_edit = new_matches.get_flag("no-edit");

        // Create the script path
        let mut script_path = get_scripts_dir();
        if !location.is_empty() {
            script_path.push(location);
        }
        // Only append .sh if the name doesn't already end with it
        let script_name = if name.ends_with(".sh") {
            name.to_string()
        } else {
            format!("{}.sh", name)
        };
        script_path.push(&script_name);

        // Create parent directories if they don't exist
        if let Some(parent) = script_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        // Create the script with basic template
        let template = format!(
            "#!/bin/bash\n#@description: {}\n#@arg:input - Input file\n#@flag:verbose - Enable verbose output\n",
            name.trim_end_matches(".sh") // Use name without extension in description
        );
        std::fs::write(&script_path, template).unwrap();

        // Make the script executable
        std::fs::set_permissions(
            &script_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();

        // Open in editor if requested
        if !no_edit {
            let editor = editor
                .map(|e| e.to_string())
                .or_else(|| env::var("EDITOR").ok())
                .unwrap_or_else(|| "vim".to_string());

            Command::new(&editor)
                .arg(&script_path)
                .status()
                .expect("Failed to open editor");
        }

        println!("Created script: {}", script_path.display());
        std::process::exit(0);
    }

    // Handle the edit command
    if let Some(("edit", edit_matches)) = matches.subcommand() {
        let components: Vec<String> = edit_matches
            .get_many::<String>("command")
            .unwrap()
            .map(|s| s.to_string())
            .collect();
        let editor = edit_matches.get_one::<String>("editor");

        // Find the script file in the command structure
        if let Some(script_path) = find_script_file(&components) {
            // Open in editor
            let editor = editor
                .map(|e| e.to_string())
                .or_else(|| env::var("EDITOR").ok())
                .unwrap_or_else(|| "vim".to_string());

            Command::new(&editor)
                .arg(&script_path)
                .status()
                .expect("Failed to open editor");

            println!("Edited script: {}", script_path.display());
            std::process::exit(0);
        } else {
            eprintln!("Script not found: {}", components.join("/"));
            std::process::exit(1);
        }
    }

    // Show help if no command is provided
    if matches.subcommand().is_none() {
        cli_for_help.print_help().unwrap();
        std::process::exit(1);
    }

    // Find the matching command and execute it
    if let Some((command, sub_m)) = matches.subcommand() {
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
            let mut dir_cli = clap::Command::new(components.join(" "));
            for cmd_with_path in shutl::command::build_command_tree(&path) {
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
}
