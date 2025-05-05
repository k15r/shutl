use crate::get_scripts_dir;
use crate::metadata::parse_command_metadata;
use clap::{Arg, Command, crate_authors, crate_description, crate_name, crate_version};
use is_executable::IsExecutable;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A command with its associated file path
pub struct CommandWithPath {
    pub command: Command,
    pub file_path: std::path::PathBuf,
}

/// Builds a command for a script file
fn build_script_command(name: std::string::String, path: &Path) -> CommandWithPath {
    let metadata = parse_command_metadata(path);
    let mut cmd = Command::new(&name).disable_help_subcommand(true);

    // Add the shutl-verbose flag
    cmd = cmd.arg(
        Arg::new("shutlverboseid")
            .help("Enable verbose output")
            .hide(true)
            .long("shutl-verbose")
            .action(clap::ArgAction::SetTrue),
    );

    // Add description if available
    if !metadata.description.is_empty() {
        cmd = cmd.about(&metadata.description);
    }

    // Add arguments
    for cmdarg in &metadata.args {
        let mut arg = Arg::new(&cmdarg.name).help(&cmdarg.description);

        if let Some(default_value) = &cmdarg.default {
            arg = arg.default_value(default_value);
        } else {
            arg = arg.required(true);
        }

        cmd = cmd.arg(arg);
    }

    // Add catch-all argument if present
    if let Some((_, description)) = &metadata.catch_all {
        cmd = cmd.arg(
            Arg::new("additional-args")
                .help(description)
                .num_args(1..)
                .required(false),
        );
    }

    // Add flags
    for flag in &metadata.flags {
        let mut arg = Arg::new(&flag.name)
            .help(&flag.description)
            .long(&flag.name);

        if flag.is_bool {
            arg = arg.action(clap::ArgAction::SetTrue);
            // Add negated version for boolean flags
            let negated_name = format!("no-{}", flag.name);
            cmd = cmd.arg(
                Arg::new(&negated_name)
                    .help(format!("Disable the '{}' flag", flag.name))
                    .long(&negated_name)
                    .action(clap::ArgAction::SetTrue),
            );
        } else {
            if let Some(default_value) = &flag.default {
                arg = arg.default_value(default_value);
            }
            if !flag.options.is_empty() {
                arg = arg.value_parser(clap::builder::PossibleValuesParser::new(&flag.options));
            }
        }

        if flag.required {
            arg = arg.required(true);
        }

        cmd = cmd.arg(arg);
    }

    CommandWithPath {
        command: cmd,
        file_path: path.to_path_buf(),
    }
}

fn trim_supported_extensions(name: &std::string::String) -> std::string::String {
    let supported_extensions = ["sh", "py", "rb", "js"];
    for ext in supported_extensions.iter() {
        let extstr = format!(".{}", ext);
        if name.ends_with(extstr.as_str()) {
            // Check if the file has only one extension
            return name
                .strip_suffix(extstr.as_str())
                .unwrap_or(name.as_str())
                .to_string();
        }
    }
    name.to_string()
}

/// Builds a list of commands from a directory
pub fn build_command_tree(dir_path: &Path, active_args: &Vec<String>) -> Vec<CommandWithPath> {
    // logic:
    // commandline: <binary> -- <binary>
    // arguments: dir_path: get_scripts_dir(), active_args: [""]
    // expected: main command, subcommands for all directories in base directory and all scripts in the base directory

    // commandline: <binary> -- <binary> <subfolder>
    // arguments: dir_path: get_scripts_dir()/subfolder, active_args: [<subfolder>]
    // expected: main command, subcommand for subfolder, subcommands for all directories in subfolder and all scripts in the subfolder

    log::debug!(
        "build_command_tree: dir_path {:?}, active_args: {:?}",
        dir_path,
        active_args
    );
    let mut commands = Vec::new();

    // pop the first argument from the active args
    // check if this is a directory or a script
    let mut active_args = active_args.clone();
    let first_arg = if active_args.is_empty() {
        "".to_string()
    } else {
        active_args.remove(0)
    };
    log::debug!(
        "build_command_tree: First arg: {:?}, active_args(rest): {:?}",
        first_arg,
        active_args
    );
    // check if the first argument is empty
    if first_arg.is_empty() {
        // if it is empty, we need to use the current directory
        return commands_for_dir(dir_path);
    }

    // check if the first argument is a directory
    let first_arg_path = dir_path.join(&first_arg);

    log::debug!("build_command_tree: First arg path: {:?}", first_arg_path);
    let is_dir = first_arg_path.is_dir();
    log::debug!("build_command_tree: Is dir: {:?}", is_dir);

    // if it is a directory, we only need to build the command tree for this directory
    if is_dir {
        // create a command for the directory
        let dir_name = first_arg_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let mut dir_cmd = dir_command(&first_arg_path, &dir_name);

        dir_cmd = add_dir_subcommands(dir_cmd, &first_arg_path, &active_args);

        commands.push(CommandWithPath {
            command: dir_cmd,
            file_path: first_arg_path,
        });
        return commands;
    }

    // check if the first argument is a script. here we need to find that would if we strip their extensions match the first argument
    let (is_script, script_path) = is_script_file(dir_path, &first_arg);
    if is_script {
        // build the command for the script
        let cmd = build_script_command(first_arg, &script_path);
        commands.push(cmd);
        return commands;
    }

    // if it is not a directory or a script, we need to build the command tree for the current directory
    build_command_tree(dir_path, &active_args)
}

fn add_dir_subcommands(
    dir_cmd: Command,
    first_arg_path: &Path,
    active_args: &Vec<String>,
) -> Command {
    let mut dir_cmd = dir_cmd.clone();
    let subcommands = build_command_tree(first_arg_path, active_args);
    for subcmd in subcommands {
        log::debug!(
            "build_command_tree: subcmd: {:?}",
            subcmd.command.get_name()
        );
        dir_cmd = dir_cmd.subcommand(subcmd.command);
    }
    dir_cmd
}

fn dir_command(path: &Path, dir_name: &String) -> Command {
    let mut dir_cmd = Command::new(dir_name).disable_help_subcommand(true);

    if let Ok(about) = fs::read_to_string(path.join(".shutl")) {
        dir_cmd = dir_cmd.about(about.trim().to_owned());
    }

    dir_cmd
}

fn commands_for_dir(dir: &Path) -> Vec<CommandWithPath> {
    let mut commands = Vec::new();
    log::debug!("commands_for_dir: {:?}", dir);

    if let Ok(entries) = fs::read_dir(dir) {
        let (mut directories, mut files): (Vec<_>, Vec<_>) = entries
            .filter_map(Result::ok)
            .partition(|entry| entry.path().is_dir());

        directories.retain(|entry| !entry.file_name().to_string_lossy().starts_with('.'));
        files.retain(|entry| {
            !entry.file_name().to_string_lossy().starts_with('.')
                && entry.path().is_file()
                && entry.path().is_executable()
        });

        let mut command_names = Vec::new();
        let mut use_extension = HashMap::new();

        for path in &directories {
            let dir_name = path.file_name().to_string_lossy().to_string();
            command_names.push(dir_name.clone());
            commands.push(CommandWithPath {
                command: dir_command(&path.path(), &dir_name),
                file_path: path.path(),
            });
        }

        for path in &files {
            let name = path.file_name().to_string_lossy().to_string();
            let clean_name = trim_supported_extensions(&name);
            if command_names.contains(&clean_name) {
                use_extension.insert(clean_name.clone(), true);
            } else {
                command_names.push(clean_name.clone());
            }
        }

        for path in files {
            let name = path.file_name().to_string_lossy().to_string();
            let clean_name = trim_supported_extensions(&name);
            let command_name = if use_extension.contains_key(&clean_name) {
                name
            } else {
                clean_name
            };
            commands.push(build_script_command(command_name, &path.path()));
        }
    }

    commands
}

fn is_script_file(dir_path: &Path, name: &str) -> (bool, PathBuf) {
    let script_path = dir_path.join(name);
    if script_path.is_file() && script_path.is_executable() {
        return (true, script_path);
    }

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && trim_supported_extensions(
                    &path.file_name().unwrap().to_string_lossy().to_string(),
                ) == name
            {
                return (path.is_executable(), path);
            }
        }
    }

    (false, PathBuf::new())
}

/// Builds the complete CLI command structure
pub fn build_cli_command() -> Command {
    let args = std::env::args().collect::<Vec<_>>();
    let binary_with_path = std::env::args().next().unwrap_or_default();
    let binary_name = binary_with_path.rsplit('/').next().unwrap_or_default();
    let is_completion = std::env::var("_CLAP_COMPLETE_INDEX").is_ok()
        && args.get(1).is_some_and(|arg| arg == "--")
        && args.get(2).is_some_and(|arg| arg == binary_name);

    let active_args = if is_completion {
        args.into_iter().skip(2).collect::<Vec<_>>()
    } else {
        args
    };

    let mut cli = Command::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .author(crate_authors!())
        .disable_help_subcommand(true);

    for cmd_with_path in build_command_tree(&get_scripts_dir(), &active_args) {
        cli = cli.subcommand(cmd_with_path.command);
    }

    cli
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::find_script_file_in_dir;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn create_test_script(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let script_path = dir.join(name);
        let mut file = File::create(&script_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        // Set executable permissions
        fs::set_permissions(
            &script_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        script_path
    }

    #[test]
    fn test_build_script_command() {
        let script_content = r#"#!/bin/bash
#@description: Test command
#@arg:input - Input file
#@flag:verbose - Enable verbose output [bool]
"#;

        let dir = tempdir().unwrap();
        let script_path = create_test_script(&dir.path(), "test.sh", script_content);
        let cmd_with_path = build_script_command("test".to_string(), &script_path);

        // Test command name
        assert_eq!(cmd_with_path.command.get_name(), "test");

        // Test description
        assert_eq!(
            cmd_with_path.command.get_about().unwrap().to_string(),
            "Test command"
        );

        // Test arguments
        let args: Vec<_> = cmd_with_path.command.get_arguments().collect();
        assert_eq!(args.len(), 4); // input, verbose, no-verbose

        // Test input argument
        let input_arg = args.iter().find(|a| a.get_id() == "input").unwrap();
        assert!(input_arg.is_required_set());
        assert_eq!(input_arg.get_help().unwrap().to_string(), "Input file");

        // Test verbose flag
        let verbose_arg = args.iter().find(|a| a.get_id() == "verbose").unwrap();
        assert!(!verbose_arg.is_required_set());
        assert_eq!(
            verbose_arg.get_help().unwrap().to_string(),
            "Enable verbose output"
        );

        // Test no-verbose flag
        let no_verbose_arg = args.iter().find(|a| a.get_id() == "no-verbose").unwrap();
        assert!(!no_verbose_arg.is_required_set());
        assert_eq!(
            no_verbose_arg.get_help().unwrap().to_string(),
            "Disable the 'verbose' flag"
        );
    }

    #[test]
    fn test_build_command_tree_for_subfolder() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create test directory structure
        // .shutl/subdir
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();

        // Create a subsubdirectory with a .shutl file
        // .shutl/subdir/test
        let subsubdir = subdir.join("test");
        fs::create_dir(&subsubdir).unwrap();
        let config_path = subsubdir.join(".shutl");
        fs::write(&config_path, "This is a test subsubdirectory").unwrap();
        // Create a script in the subsubdirectory
        // .shutl/subdir/test/subsubdir.sh
        create_test_script(
            &subsubdir,
            "subsubdir.sh",
            "#!/bin/bash\n#@description: Test script in subsubdirectory",
        );

        // Create test scripts
        // .shutl/subdir/test.sh
        create_test_script(
            &subdir,
            "test.sh",
            "#!/bin/bash\n#@description: subdir script",
        );

        let commands = build_command_tree(&scripts_dir, &vec!["subdir".to_string()]);

        // Test root level command
        assert_eq!(commands.len(), 1); // root.sh and subdir
        // Test subdirectory command
        let subdir_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "subdir")
            .unwrap();
        let subdir_subcmds: Vec<_> = subdir_cmd.command.get_subcommands().collect();

        for subcmd in &subdir_subcmds {
            println!("Subcommand: {}", subcmd.get_name());
        }
        assert_eq!(subdir_subcmds.len(), 2);

        // Find the test command in the subdirectory
        let testdir_cmd = subdir_subcmds
            .iter()
            .find(|c| c.get_name() == "test")
            .unwrap();
        let testdir_subcmds: Vec<_> = testdir_cmd.get_subcommands().collect();
        assert_eq!(testdir_subcmds.len(), 0);
        // Find the test.sh command in the subdirectory
        let testscript_cmd = subdir_subcmds
            .iter()
            .find(|c| c.get_name() == "test.sh")
            .unwrap();
        let testscript_subcmds: Vec<_> = testscript_cmd.get_subcommands().collect();
        assert_eq!(testscript_subcmds.len(), 0);
    }

    #[test]
    fn test_build_command_tree_for_root() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create test directory structure
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();

        // Create test scripts
        create_test_script(
            &scripts_dir,
            "root.sh",
            "#!/bin/bash\n#@description: Root script",
        );
        create_test_script(&subdir, "sub.sh", "#!/bin/bash\n#@description: Sub script");

        let commands = build_command_tree(&scripts_dir, &vec![]);

        // Test root level command
        assert_eq!(commands.len(), 2); // root.sh and subdir
        let root_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "root")
            .unwrap();
        assert_eq!(
            root_cmd.command.get_about().unwrap().to_string(),
            "Root script"
        );

        // Test subdirectory command
        let subdir_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "subdir")
            .unwrap();
        let subdir_subcmds: Vec<_> = subdir_cmd.command.get_subcommands().collect();
        assert_eq!(subdir_subcmds.len(), 0);
    }

    #[test]
    fn test_build_command_tree_directory_description() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create a subdirectory
        let subdir = scripts_dir.join("test_dir");
        fs::create_dir(&subdir).unwrap();

        // Create a directory with a .shutl file
        let config_path = subdir.join(".shutl");
        fs::write(&config_path, "This is a test directory").unwrap();

        // Create a script in the directory
        create_test_script(
            &subdir,
            "test.sh",
            "#!/bin/bash\n#@description: Test script",
        );

        let commands = build_command_tree(&scripts_dir, &vec!["test_dir".to_string()]);

        // Test directory command
        assert_eq!(commands.len(), 1); // only the directory command
        let dir_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "test_dir")
            .unwrap();
        assert_eq!(
            dir_cmd.command.get_about().unwrap().to_string(),
            "This is a test directory"
        );
    }

    #[test]
    fn test_build_command_tree_ignores_hidden() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create visible and hidden directories
        let visible_dir = scripts_dir.join("visible");
        let hidden_dir = scripts_dir.join(".hidden");
        fs::create_dir(&visible_dir).unwrap();
        fs::create_dir(&hidden_dir).unwrap();

        // Create visible and hidden scripts
        create_test_script(
            &scripts_dir,
            "visible_script.sh",
            "#!/bin/bash\n#@description: Visible script",
        );
        create_test_script(
            &scripts_dir,
            ".hidden_script.sh",
            "#!/bin/bash\n#@description: Hidden script",
        );

        let commands = build_command_tree(&scripts_dir, &vec![]);

        // Test that only visible items are included
        assert_eq!(commands.len(), 2); // visible.sh and visible directory
        let visible_cmd = commands
            .iter()
            .find(|c| c.command.get_name() == "visible")
            .unwrap();
        let visible_subcmds: Vec<_> = visible_cmd.command.get_subcommands().collect();
        assert_eq!(visible_subcmds.len(), 0);

        // Verify hidden items are not included
        assert!(
            commands
                .iter()
                .find(|c| c.command.get_name() == ".hidden")
                .is_none()
        );
        assert!(
            commands
                .iter()
                .find(|c| c.command.get_name() == ".hidden.sh")
                .is_none()
        );
    }

    #[test]
    fn test_new_command_script_names() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Test creating script without .sh extension
        let script1 = create_test_script(
            &scripts_dir,
            "test1.sh",
            "#!/bin/bash\n#@description: test1",
        );
        assert_eq!(script1.file_name().unwrap().to_str().unwrap(), "test1.sh");

        // Test creating script with .sh extension
        let script2 = create_test_script(
            &scripts_dir,
            "test2.sh.sh",
            "#!/bin/bash\n#@description: test2",
        );
        assert_eq!(
            script2.file_name().unwrap().to_str().unwrap(),
            "test2.sh.sh"
        );

        // Test creating script in subdirectory
        let subdir = scripts_dir.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let script3 = create_test_script(&subdir, "test3.sh", "#!/bin/bash\n#@description: test3");
        assert_eq!(script3.file_name().unwrap().to_str().unwrap(), "test3.sh");

        // Verify all scripts are executable
        assert_ne!(script1.metadata().unwrap().permissions().mode() & 0o111, 0);
        assert_ne!(script2.metadata().unwrap().permissions().mode() & 0o111, 0);
        assert_ne!(script3.metadata().unwrap().permissions().mode() & 0o111, 0);

        // Verify script contents
        let content1 = fs::read_to_string(&script1).unwrap();
        assert!(content1.contains("#@description: test1"));
        let content2 = fs::read_to_string(&script2).unwrap();
        assert!(content2.contains("#@description: test2"));
        let content3 = fs::read_to_string(&script3).unwrap();
        assert!(content3.contains("#@description: test3"));
    }

    #[test]
    fn test_colliding_folder_and_script() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();
        // Create a directory with the same name as a script
        let subdir = scripts_dir.join("test");
        fs::create_dir(&subdir).unwrap();
        // Create a script with the same name as the directory
        create_test_script(
            &scripts_dir,
            "test.sh",
            "#!/bin/bash\n#@description: Test script",
        );

        // Create a script in the subdirectory to test collision
        create_test_script(
            &subdir,
            "subdirtest.sh",
            "#!/bin/bash\n#@description: Test script in subdirectory",
        );

        let commands = build_command_tree(&scripts_dir, &vec![]);

        // Verify both commands exist with different names
        assert_eq!(commands.len(), 2);

        // First script should have name "test"
        let cmd1 = commands
            .iter()
            .find(|c| c.command.get_name() == "test.sh")
            .unwrap();
        assert_eq!(cmd1.command.get_about().unwrap().to_string(), "Test script");
    }

    #[test]
    fn test_duplicate_script_names() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create two scripts with similar names
        create_test_script(
            &scripts_dir,
            "test.sh",
            "#!/bin/bash\n#@description: First test script",
        );
        create_test_script(
            &scripts_dir,
            "test.sh.sh",
            "#!/bin/bash\n#@description: Second test script",
        );

        let commands = build_command_tree(&scripts_dir, &vec![]);

        // Verify both commands exist with different names
        assert_eq!(commands.len(), 2);

        // First script should have name "test"
        let cmd1 = commands
            .iter()
            .find(|c| c.command.get_name() == "test")
            .unwrap();
        assert_eq!(
            cmd1.command.get_about().unwrap().to_string(),
            "First test script"
        );

        // Second script should have name "test.sh"
        let cmd2 = commands
            .iter()
            .find(|c| c.command.get_name() == "test.sh")
            .unwrap();
        assert_eq!(
            cmd2.command.get_about().unwrap().to_string(),
            "Second test script"
        );
    }

    #[test]
    fn test_edit_command() {
        let dir = tempdir().unwrap();
        let scripts_dir = dir.path().join(".shutl");
        fs::create_dir(&scripts_dir).unwrap();

        // Create a nested directory structure
        let subdir = scripts_dir.join("subdir");
        let subsubdir = subdir.join("subsubdir");
        fs::create_dir_all(&subsubdir).unwrap();

        // Create test scripts at different levels
        let root_script = create_test_script(
            &scripts_dir,
            "root.sh",
            "#!/bin/bash\n#@description: Root script",
        );
        let sub_script =
            create_test_script(&subdir, "sub.sh", "#!/bin/bash\n#@description: Sub script");
        let subsub_script = create_test_script(
            &subsubdir,
            "subsub.sh",
            "#!/bin/bash\n#@description: Sub-sub script",
        );

        // Test editing root script
        let components = vec!["root".to_string()];
        assert_eq!(
            find_script_file_in_dir(&components, &scripts_dir).unwrap(),
            root_script
        );

        // Test editing script in subdirectory
        let components = vec!["subdir".to_string(), "sub".to_string()];
        assert_eq!(
            find_script_file_in_dir(&components, &scripts_dir).unwrap(),
            sub_script
        );

        // Test editing script in nested directory
        let components = vec![
            "subdir".to_string(),
            "subsubdir".to_string(),
            "subsub".to_string(),
        ];
        assert_eq!(
            find_script_file_in_dir(&components, &scripts_dir).unwrap(),
            subsub_script
        );

        // Test non-existent script
        let components = vec!["nonexistent".to_string()];
        assert!(find_script_file_in_dir(&components, &scripts_dir).is_none());

        // Test non-existent script in existing directory
        let components = vec!["subdir".to_string(), "nonexistent".to_string()];
        assert!(find_script_file_in_dir(&components, &scripts_dir).is_none());
    }
}
