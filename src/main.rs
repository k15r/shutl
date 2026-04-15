use clap::ArgMatches;
use shutl::builtin;
use shutl::{build_cli_command, execute_script, find_script_file, get_scripts_dir};

fn main() {
    env_logger::builder().init();

    log::debug!("args: {:?}", std::env::args().collect::<Vec<_>>());

    clap_complete::CompleteEnv::with_factory(build_cli_command).complete();

    let cli = build_cli_command();
    let mut cli_for_help = cli.clone();
    let matches = cli.get_matches();

    match matches.subcommand() {
        Some(("new", sub_matches)) => builtin::handle_new(sub_matches),
        Some(("edit", sub_matches)) => builtin::handle_edit(sub_matches),
        Some(("list", sub_matches)) => builtin::handle_list(sub_matches),
        Some(("validate", sub_matches)) => builtin::handle_validate(sub_matches),
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
