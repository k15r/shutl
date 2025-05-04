use clap::ArgMatches;
use shutl::{build_cli_command, execute_script, find_script_file, get_scripts_dir};
use std::env;
use std::process::Command;
use std::string::String;

fn main() {
    env_logger::builder().init();

    let args: Vec<String> = std::env::args().collect();

    // write the args to a file all args on one line
    log::debug!("args: {:?}", args);

    clap_complete::CompleteEnv::with_factory(build_cli_command).complete();
    // Build the CLI command structure
    let mut cli = build_cli_command();

    cli = add_new_and_edit_cmd(&mut cli);

    let mut cli_for_help = cli.clone();

    // Get matches for command processing
    let matches = cli.get_matches();

    // Handle the new command
    if let Some(("new", new_matches)) = matches.subcommand() {
        handle_new(new_matches);
    }

    // Handle the edit command
    if let Some(("edit", edit_matches)) = matches.subcommand() {
        handle_edit(edit_matches);
    }

    // Show help if no command is provided
    if matches.subcommand().is_none() {
        cli_for_help.print_help().unwrap();
        std::process::exit(1);
    }

    // Find the matching command and execute it
    if let Some((command, sub_m)) = matches.subcommand() {
        execute_command(command, sub_m);
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

fn handle_edit(edit_matches: &ArgMatches) {
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

fn handle_new(new_matches: &ArgMatches) {
    let name = new_matches.get_one::<String>("name").unwrap();
    let location = new_matches.get_one::<String>("location").unwrap();
    let editor = new_matches.get_one::<String>("editor");
    let no_edit = new_matches.get_flag("no-edit");

    // Build the script path
    let mut script_path = get_scripts_dir();
    if !location.is_empty() {
        script_path.push(location);
    }
    let with_extension = &format!("{}.sh", name);
    script_path.push(if name.ends_with(".sh") {
        name
    } else {
        with_extension
    });

    // Ensure parent directories exist
    if let Some(parent) = script_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    // Write the script template
    let template = format!(
        "#!/bin/bash\n#@description: {}\n#@arg:input - Input file\n#@flag:verbose - Enable verbose output\n",
        name.trim_end_matches(".sh")
    );
    std::fs::write(&script_path, template).unwrap();

    // Make the script executable
    std::fs::set_permissions(
        &script_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    // Open the script in an editor if required
    if !no_edit {
        let editor = editor
            .cloned()
            .or_else(|| env::var("EDITOR").ok())
            .unwrap_or_else(|| "vim".to_string());

        Command::new(editor)
            .arg(&script_path)
            .status()
            .expect("Failed to open editor");
    }

    println!("Created script: {}", script_path.display());
    std::process::exit(0);
}

fn add_new_and_edit_cmd(cli: &mut clap::Command) -> clap::Command {
    // Add the new command
    let mut cli_cmd = cli.clone();
    cli_cmd = cli_cmd.subcommand(
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
                clap::Arg::new("type")
                    .help("Type of script (e.g., bash, python)")
                    .long("type")
                    .short('t')
                    .value_parser(clap::builder::PossibleValuesParser::new(vec![
                        "zsh", "bash", "python", "ruby", "node",
                    ]))
                    .default_value("zsh"),
            )
            .arg(
                clap::Arg::new("no-edit")
                    .help("Don't open the script in an editor")
                    .long("no-edit")
                    .action(clap::ArgAction::SetTrue),
            ),
    );

    // Add the edit command
    cli_cmd = cli_cmd.subcommand(
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
    cli_cmd
}
