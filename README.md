# ![Shutl Logo](assets/logo-xs.png) Shutl

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

A command-line tool for organizing, managing, and executing scripts as commands.
Using command completion it provides a user-friendly interface for running shell scripts with metadata-driven arguments and flags.

[![asciicast](https://asciinema.org/a/710656.svg)](https://asciinema.org/a/710656)

## Features

- **Dynamic Command Generation**: Automatically creates CLI commands from shell scripts
- **Metadata Support**: Use special comments in your shell scripts to define command metadata
- **Flexible Argument Handling**: Support for required and optional arguments with defaults
- **Boolean Flags**: Automatic generation of boolean flags with negated versions
- **Catch-all Arguments**: Support for additional arguments beyond defined parameters
- **Directory-based Organization**: Organize commands in directories for better structure

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
#@arg:... - Additional arguments

# Your script logic here
if [ "$SHUTL_DRY_RUN" = true ]; then
  echo "Dry run mode enabled"
fi

echo "Hostname: ${SHUTL_HOST}"

echo "Processing input file: $SHUTL_INPUT"
echo "Output will be saved to: $SHUTL_OUTPUT"

# Handle additional arguments
if [ "$#" -gt 0 ]; then
  echo "Additional arguments: $SHUTL_ADDITIONAL_ARGS"
fi
```

### Command Completion

To enable command completion, add the following to your shell configuration file (e.g., `.bashrc`, `.zshrc`):

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
| Catch-all    | `#@arg:... - Additional arguments description`                                        |
| Flags        | `#@flag:name - Flag with default value [default:value]`                               |
| Flags        | `#@flag:name - Boolean flag [bool]`                                                   |
| Flags        | `#@flag:name - Flag with allowed values [options:allowed-value\|other-allowed-value]` |
| Flags        | `#@flag:name - Required Flag [required]`                                              |
| Flags        | `#@flag:name - Flag with file as value [file]`                                        |
| Flags        | `#@flag:name - Flag with directory as value [dir]`                                    |
| Flags        | `#@flag:name - Flag with anypath as value [path]`                                     |

### Running Commands

```bash
# Basic usage
shutl example-command --input file.txt

# With flags
shutl example-command --input file.txt --host example.com --dry-run

# Using negated flags
shutl example-command --input file.txt --no-dry-run
```

## Project Structure

```bash
shutl/
├── src/              # Rust source code
└── Cargo.toml        # Project dependencies

# Scripts are stored in:
~/.shutl/            # User's scripts directory
├── command1.sh
└── subdir/
    └── command2.sh
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details. 
