# ![Shutl Logo](assets/logo-xs.png) Shutl

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

A command-line tool for organizing, managing, and executing scripts as commands.
Using command completion, it provides a user-friendly interface for running shell scripts with metadata-driven arguments and flags.

[![asciicast](https://asciinema.org/a/710656.svg)](https://asciinema.org/a/710656)

## Features

- **Dynamic Command Generation**: Automatically creates CLI commands from shell scripts
- **Metadata Support**: Supports special comments in your shell scripts to define command metadata
- **Flexible Argument Handling**: Supports required and optional arguments with defaults
- **Boolean Flags**: Automatically generates Boolean flags with negated versions
- **Catch-all Arguments**: Supports additional arguments beyond defined parameters
- **Directory-based Organization**: Organizes commands in directories for better structure

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/k15r/shutl.git
cd shutl

# Build the project
cargo build --release
```

### Using HomeBrew

```bash
brew tap k15r/shutl
brew install shutl
```

## Usage

### Writing Scripts

Create shell scripts in the `~/.shutl` directory with metadata comments:

```bash
#!/bin/bash
#@description: Example command with various metadata
#@arg:input - Input file path
#@arg:output - Output file path [default:output.txt]
#@flag:host - Host name [default:localhost]
#@flag:dry-run - Perform a dry run [bool,default:false]
#@arg:...files - Additional files to process

# Your script logic here
if [ "$SHUTL_DRY_RUN" = "true" ]; then
  echo "Dry run mode enabled"
fi

echo "Hostname: ${SHUTL_HOST}"

echo "Processing input file: $SHUTL_INPUT"
echo "Output will be saved to: $SHUTL_OUTPUT"

# Handle additional files
if [ -n "$SHUTL_FILES" ]; then
  echo "Additional files: $SHUTL_FILES"
fi
```

### Command Completion

To enable command completion, add the following to your shell configuration file (like `.bashrc` or `.zshrc`):

#### bash:

```bash
. <(COMPLETE=bash shutl)
```

#### zsh:

```bash
. <(COMPLETE=zsh shutl)
``` 

### Metadata Syntax

| **Metadata** | **Syntax**                                                                            |
|--------------|---------------------------------------------------------------------------------------|
| Description  | `#@description: Your command description`                                             |
| Arguments    | `#@arg:name - Argument description`                                                   |
| Arguments    | `#@arg:name - Required argument with default [default:value]`                         |
| Arguments    | `#@arg:name - Argument with allowed values [options:val1\|val2]`                      |
| Catch-all    | `#@arg:... - Additional arguments description`                                        |
| Catch-all    | `#@arg:...name - Named catch-all arguments`                                            |
| Catch-all    | `#@arg:...files - Required named catch-all [required]`                                 |
| Flags        | `#@flag:name - Flag with default value [default:value]`                               |
| Flags        | `#@flag:name - Boolean flag [bool]`                                                   |
| Flags        | `#@flag:name - Flag with allowed values [options:allowed-value\|other-allowed-value]` |
| Flags        | `#@flag:name - Required Flag [required]`                                              |
| Flags        | `#@flag:name - Flag with file completion [file]`                                      |
| Flags        | `#@flag:name - Flag with file completion from directory [file:~/path]`                |
| Flags        | `#@flag:name - Flag with file completion with env override [file:~/path:ENV_VAR]`     |
| Flags        | `#@flag:name - Flag with directory completion [dir]`                                  |
| Flags        | `#@flag:name - Flag with directory completion from directory [dir:~/path]`            |
| Flags        | `#@flag:name - Flag with directory completion with env override [dir:~/path:ENV_VAR]` |
| Flags        | `#@flag:name - Flag with any path completion [path]`                                  |
| Flags        | `#@flag:name - Flag with any path completion from directory [path:~/path]`            |
| Flags        | `#@flag:name - Flag with any path completion with env override [path:~/path:ENV_VAR]` |

Positional arguments (`#@arg:`) are required by default unless they have a `default` value or are catch-all (`#@arg:...`). Catch-all arguments are optional by default but can be made required with `[required]`. Setting both `required` and `default` is contradictory -- `required` will be ignored.

The `file`, `dir`, and `path` annotations support an optional environment variable override. If the env var is set, it will be used instead of the default path for shell completion. Example:

```bash
#@flag:config - Configuration file [file:~/.config/myapp:MYAPP_CONFIG_DIR]
# Completes files from $MYAPP_CONFIG_DIR if set, otherwise ~/.config/myapp
```

### Running Commands

Basic usage:

```bash
shutl example-command --input file.txt
```
Using flags:

```bash
shutl example-command --input file.txt --host example.com --dry-run
```

Using negated flags:

```bash
shutl example-command --input file.txt --no-dry-run
```

## Built-in Commands

### Creating a New Script

```bash
shutl new <location> <name> [--editor <editor>] [--type <type>] [--no-edit]
```

- `location`: Directory relative to `~/.shutl` (supports tab completion)
- `name`: Script name (without .sh extension)
- `--editor`, `-e`: Editor to use (defaults to `$EDITOR` or `vim`)
- `--type`, `-t`: Script type: `zsh`, `bash` (default: `zsh`)
- `--no-edit`: Don't open the script in an editor after creation

Example:
```bash
shutl new tools deploy --type bash
```

### Editing an Existing Script

```bash
shutl edit <command...> [--editor <editor>]
```

- `command`: Command path components (e.g., `subdir myscript`)
- `--editor`, `-e`: Editor to use (defaults to `$EDITOR` or `vim`)

Example:
```bash
shutl edit tools deploy
```

## Environment Variables

- `SHUTL_DIR`: Override the default scripts directory (`~/.shutl`)

## Project Structure

```bash
shutl/
├── src/              # Rust source code
└── Cargo.toml        # Project dependencies

# Scripts are stored in:
~/.shutl/            # User's scripts directory (or $SHUTL_DIR)
├── command1.sh
└── subdir/
    ├── .shutl       # Optional: directory description shown in help
    └── command2.sh
```

### Directory Descriptions

You can add a `.shutl` file in any directory to provide a description that appears in the help output:

```bash
# Create a directory with a description
mkdir -p ~/.shutl/deploy
echo "Deployment scripts for various environments" > ~/.shutl/deploy/.shutl
```

## Contributing

Contributions are welcome! Please feel free to submit a pull request.

## License

This project is licensed under the MIT License - see the LICENSE file for details. 
