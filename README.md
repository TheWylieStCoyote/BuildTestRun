# MakeBuildRun

`mbr` is a Rust CLI for running project-defined commands from a hidden project config file.

## What It Does

Each project contains a `.mbr.toml` file that defines how to build, test, run, lint, analyze, and execute other named commands. The CLI discovers that file automatically and executes the configured command for the action you choose.

## Commands

```bash
mbr build
mbr test
mbr run
mbr exec lint
mbr validate
mbr init
mbr list
mbr which
mbr doctor
mbr fmt
mbr clean
mbr ci
```

## Config File

Create a `.mbr.toml` file at the root of your project:

```toml
[project]
name = "example"
root = "."

[env]
RUST_LOG = "info"

[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"] }
```

## Behavior

- Finds `.mbr.toml` in the current directory or a parent directory
- Runs the matching command for `build`, `test`, or `run`
- Supports named commands via `mbr exec <name>`
- Forwards extra arguments after `--`
- Validates config with `mbr validate`
- Generates a starter config with `mbr init`
- Lists commands with `mbr list`
- Shows resolved config with `mbr which`
- Checks for common issues with `mbr doctor`
- Supports `--dry-run` for execution commands
- Uses the configured project root when provided
- Passes config environment variables to the child process
- Streams command output directly to the terminal

## Development

Build and test the project with:

```bash
cargo test
```

## Documentation

- `SPEC.md` describes the project in detail
- `EXAMPLES.md` contains ready-to-use config examples

## Example Projects

- Rust: `cargo build`, `cargo test`, `cargo run`
- Node: `npm run build`, `npm test`, `npm start`
- Python: `python -m build`, `pytest`, `python main.py`
- CMake: `cmake -S . -B build`, `ctest --test-dir build`, `./build/native-app`

## Common Extensions

- Lint: `mbr exec lint`
- Analysis: `mbr exec analyze`
- CI: `mbr exec ci`
- Format: `mbr fmt`
- Clean: `mbr clean`
- CI alias: `mbr ci`
