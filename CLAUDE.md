# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

```bash
cargo build              # Build debug version
cargo build --release    # Build release version
cargo test               # Run all tests
cargo test test_name     # Run a single test by name
cargo run                # Run the CLI tool
```

## Architecture

Shutl is a Rust CLI tool that dynamically generates commands from shell scripts (bash/zsh only) stored in `~/.shutl` (or `$SHUTL_DIR`). It parses metadata comments in scripts to create typed CLI arguments with shell completion support.

### Core Modules

- **`lib.rs`** - Entry point, exports public API, provides `get_scripts_dir()` utility
- **`metadata.rs`** - Parses script metadata comments (`#@description:`, `#@arg:`, `#@flag:`) into `CommandMetadata` struct containing `LineType` variants (Description, Flag, Positional)
- **`command.rs`** - Builds clap `Command` tree from scripts directory structure. `build_command_tree()` recursively creates subcommands from directories and scripts
- **`script.rs`** - Executes scripts by setting `SHUTL_*` environment variables from parsed arguments. Also provides `find_script_file()` for locating scripts by command path components
- **`main.rs`** - CLI entry point, uses match dispatch for `new`, `edit`, and script commands. Built-in `new` and `edit` commands are defined here (not in `command.rs`)

### Key Design Patterns

**Metadata parsing flow**: Script file → `parse_command_metadata()` → `LineType` enum → clap `Arg` configuration

**Argument to environment variable mapping**: Arguments become `SHUTL_<NAME>` env vars (e.g., `--dry-run` → `SHUTL_DRY_RUN`)

**Script discovery**: Directory structure mirrors command hierarchy. A script at `~/.shutl/foo/bar.sh` becomes `shutl foo bar`. Scripts are matched by file stem (without extension). Directories can have `.shutl` files for descriptions.

**Name collision handling**: When a directory and script share the same stem (e.g., `test/` and `test.sh`), the script keeps its full filename including extension to avoid ambiguity.

**Completion**: Uses `clap_complete` with `CompleteEnv` for dynamic shell completion. Path-type arguments (`file`, `dir`, `path`) get `PathCompleter` integration.

### Metadata Syntax in Scripts

```bash
#@description: Command description
#@arg:name - Description [default:value]
#@arg:name - Argument description [required]
#@arg:... - Catch-all for additional args (env var: SHUTL_ADDITIONAL_ARGS)
#@arg:...name - Named catch-all (e.g. ...files → env var: SHUTL_FILES)
#@arg:...name - Required named catch-all [required]
#@flag:name - Description [bool,default:false]
#@flag:name - Description [options:opt1|opt2]
#@flag:name - Description [required]
#@flag:name - Description [file] or [file:~/start/path]
#@flag:name - Description [dir] or [dir:~/start/path]
#@flag:name - Description [path] or [path:~/start/path]
```

Annotations in brackets: `bool`, `required`, `default:`, `options:`, `file:`, `dir:`, `path:`

Notes:
- This tool is shell-only (bash/zsh). Do not add support for other languages.
- `file`, `dir`, and `path` only affect shell completion
- They optionally accept a start directory for completions
- Positional args (`#@arg:`) are required by default unless they have a `default` value or are catch-all (`#@arg:...`)
- Catch-all args are optional by default but can be made required with `[required]`
- Setting both `required` and `default` is contradictory - `required` will be ignored
- Bool flags automatically generate a `--no-<name>` counterpart

### Hidden Debug Flags

Scripts automatically get `--shutl-verbose` and `--shutl-noexec` flags for debugging argument passing.
