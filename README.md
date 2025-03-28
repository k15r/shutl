# Shutl

A powerful CLI tool that dynamically maps commands to shell scripts, making it easy to create and manage command-line tools from shell scripts.

## Features

- **Dynamic Command Generation**: Automatically creates CLI commands from shell scripts
- **Metadata Support**: Use special comments in your shell scripts to define command metadata
- **Flexible Argument Handling**: Support for required and optional arguments with defaults
- **Boolean Flags**: Automatic generation of boolean flags with negated versions
- **Catch-all Arguments**: Support for additional arguments beyond defined parameters
- **Directory-based Organization**: Organize commands in directories for better structure

## Installation

```bash
# Clone the repository
git clone https://github.com/k15r/shutl.git
cd shutl

# Build the project
cargo build --release
```

## Usage

### Writing Scripts

Create shell scripts in the `~/.shutl` directory with metadata comments:

```bash
#!/bin/bash
#@description: My awesome command
#@arg:input - Input file path
#@flag:verbose - Enable verbose output
#@flag:force - Force the operation (default: false)

# Your script logic here
```

### Metadata Syntax

- **Description**: `#@description: Your command description`
- **Arguments**: `#@arg:name - Argument description`
- **Flags**: `#@flag:name - Flag description (default: value)`
- **Catch-all**: `#@catch-all: Additional arguments description`

### Running Commands

```bash
# Basic usage
shutl my-command --input file.txt

# With flags
shutl my-command --verbose --force

# Using negated flags
shutl my-command --no-verbose
```

## Project Structure

```
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
