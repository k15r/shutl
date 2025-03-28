use clap::{Arg, Command, CommandFactory};
use clap_complete::{generate, Generator, Shell};
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
use walkdir::WalkDir;
use regex::Regex;

/// The directory where scripts are stored
const SCRIPTS_DIR: &str = "scripts";

/// Metadata for a command parsed from its shell script
#[derive(Default)]
struct CommandMetadata {
    description: String,
    args: Vec<(String, String)>, // (name, description)
    flags: Vec<(String, String, bool)>, // (name, description, required)
}

/// Executes a shell script with the provided arguments
fn execute_script(script_path: &Path, matches: &clap::ArgMatches) -> std::io::Result<()> {
    // Build the command with the script path
    let mut command = ProcessCommand::new("bash");
    command.arg(script_path);

    // Add positional arguments in order
    for (arg_name, _) in parse_command_metadata(script_path).args {
        if let Some(value) = matches.get_one::<String>(&arg_name) {
            command.arg(value);
        }
    }

    // Add flags
    for (flag_name, _, _) in parse_command_metadata(script_path).flags {
        if matches.contains_id(&flag_name) {
            command.arg(format!("--{}", flag_name));
        }
    }

    // Execute the command
    let status = command.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Recursively finds a script file in the scripts directory
fn find_script_file(components: &[String]) -> Option<std::path::PathBuf> {
    let mut path = Path::new(SCRIPTS_DIR).to_path_buf();
    
    // Add all components except the last one as directories
    for component in components.iter().take(components.len() - 1) {
        path.push(component);
    }
    
    // Add the last component as a file
    path.push(components.last().unwrap());
    
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Parses command metadata from a shell script
fn parse_command_metadata(path: &Path) -> CommandMetadata {
    let mut metadata = CommandMetadata::default();
    
    if let Ok(contents) = fs::read_to_string(path) {
        // Look for metadata in consecutive comment lines at the start of the file
        let lines: Vec<_> = contents.lines().collect();
        let mut i = 0;
        
        // Skip shebang if present
        if lines.first().map_or(false, |line| line.starts_with("#!")) {
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
                if let Some((name, desc)) = arg_line.split_once('-') {
                    let name = name.trim().to_string();
                    let desc = desc.trim().to_string();
                    metadata.args.push((name, desc));
                }
            }
            
            // Parse flags
            if line.starts_with("flag:") {
                let flag_line = line.replace("flag:", "").trim().to_string();
                if let Some((name, desc)) = flag_line.split_once('-') {
                    let name = name.trim().to_string();
                    let desc = desc.trim().to_string();
                    let required = desc.contains("required");
                    let description = if required {
                        desc.replace("required", "").trim().to_string()
                    } else {
                        desc
                    };
                    metadata.flags.push((name, description, required));
                }
            }
            
            i += 1;
        }
    }
    
    metadata
}

/// Recursively discovers available commands and their metadata
fn discover_commands_metadata() -> Vec<(Vec<String>, CommandMetadata)> {
    let mut commands = Vec::new();
    
    if let Ok(entries) = fs::read_dir(SCRIPTS_DIR) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                discover_commands_in_dir(&path, &mut commands, Vec::new());
            } else if path.is_file() {
                let metadata = parse_command_metadata(&path);
                commands.push((vec![path.file_name().unwrap().to_string_lossy().to_string()], metadata));
            }
        }
    }
    
    commands
}

/// Recursively discovers commands in a directory
fn discover_commands_in_dir(dir_path: &Path, commands: &mut Vec<(Vec<String>, CommandMetadata)>, mut current_path: Vec<String>) {
    let dir_name = dir_path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    current_path.push(dir_name);
    
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                discover_commands_in_dir(&path, commands, current_path.clone());
            } else if path.is_file() {
                let metadata = parse_command_metadata(&path);
                let mut command_path = current_path.clone();
                command_path.push(path.file_name().unwrap().to_string_lossy().to_string());
                commands.push((command_path, metadata));
            }
        }
    }
}

/// Builds a command tree from path components and metadata
fn build_command_tree(path_components: &[String], metadata: &CommandMetadata) -> Command {
    let mut cmd = Command::new(path_components.last().unwrap());
    
    // Add description if available
    if !metadata.description.is_empty() {
        cmd = cmd.about(&metadata.description);
    }
    
    // Add arguments
    for (name, description) in &metadata.args {
        cmd = cmd.arg(Arg::new(name)
            .help(description)
            .required(true));
    }
    
    // Add flags
    for (name, description, required) in &metadata.flags {
        cmd = cmd.arg(Arg::new(name)
            .help(description)
            .long(name)
            .required(*required));
    }
    
    // Build parent commands in reverse order
    let mut current_cmd = cmd;
    for component in path_components.iter().rev().skip(1) {
        let mut parent = Command::new(component);
        parent = parent.subcommand(current_cmd);
        current_cmd = parent;
    }
    
    current_cmd
}

/// Generates completion script for a specific shell
fn generate_completion_script(shell: Shell, output_path: Option<&str>) -> std::io::Result<()> {
    // Create a temporary command structure for completion generation
    let mut cmd = Command::new("cli")
        .about("A CLI tool that dynamically maps commands to shell scripts");
    
    // Add the completion subcommand
    cmd = cmd.subcommand(Command::new("completion")
        .about("Generate shell completion scripts")
        .arg(Arg::new("shell")
            .help("The shell to generate completions for")
            .required(true)
            .value_parser(["bash", "zsh", "fish", "elvish", "powershell"]))
        .arg(Arg::new("output")
            .help("Output file to write the completion script to")
            .long("output")
            .short('o')));
    
    // Dynamically discover and add all commands
    for (path_components, metadata) in discover_commands_metadata() {
        let subcmd = build_command_tree(&path_components, &metadata);
        cmd = cmd.subcommand(subcmd);
    }
    
    // Generate the completion script
    if let Some(output_path) = output_path {
        let mut file = std::fs::File::create(output_path)?;
        generate(shell, &mut cmd, "cli", &mut file);
        println!("Completion script written to {}", output_path);
    } else {
        generate(shell, &mut cmd, "cli", &mut std::io::stdout());
    }
    
    Ok(())
}

/// Builds the complete CLI command structure
fn build_cli_command() -> Command {
    let mut cli = Command::new("cli")
        .about("A CLI tool that dynamically maps commands to shell scripts")
    
    // Add all discovered commands
    for (path_components, metadata) in discover_commands_metadata() {
        let subcmd = build_command_tree(&path_components, &metadata);
        cli = cli.subcommand(subcmd);
    }
    
    cli
}

fn main() {
    clap_complete::CompleteEnv::with_factory(build_cli_command).complete();
    // Build the CLI command structure
    let cli = build_cli_command();
    
    // Get matches for command processing
    let matches = cli.get_matches();
    
    // Find the matching command and execute it
    if let Some((command, sub_m)) = matches.subcommand() {
        // Collect all command components
        let mut components = vec![command.to_string()];
        let mut current = sub_m;
        while let Some((subcommand, sub_matches)) = current.subcommand() {
            components.push(subcommand.to_string());
            current = sub_matches;
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
