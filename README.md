# MakeBuildRun

`mbr` is a Rust CLI for running `build`, `test`, and `run` commands from a hidden project config file.

## What It Does

Each project contains a `.mbr.toml` file that defines how to build, test, and run that project. The CLI discovers that file automatically and executes the configured command for the action you choose.

## Commands

```bash
mbr build
mbr test
mbr run
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
build = "cargo build"
test = "cargo test"
run = "cargo run"
```

## Behavior

- Finds `.mbr.toml` in the current directory or a parent directory
- Runs the matching command for `build`, `test`, or `run`
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
