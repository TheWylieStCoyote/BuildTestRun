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
mbr validate --strict
mbr init --template node
mbr list
mbr list --verbose
mbr which
mbr doctor --strict
mbr show build
mbr explain build
mbr --workspace workspace build
mbr parallel fmt lint test
mbr workspace --list
mbr workspace build
mbr package --output demo.tar.gz
mbr completions bash
mbr manpage
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
docs = { program = "cargo", args = ["doc"], cwd = "docs" }
check = { program = "cargo", args = ["check"], timeout = 60 }
```

## Behavior

- Finds `.mbr.toml` in the current directory or a parent directory
- Merges parent `.mbr.toml` files with child overrides
- Runs the matching command for `build`, `test`, or `run`
- Supports named commands via `mbr exec <name>`
- Forwards extra arguments after `--`
- Validates config with `mbr validate`
- Supports `--strict` for `validate` and `doctor`
- Generates a starter config with `mbr init --template <name>`
- Starter templates include rust, node, pnpm, yarn, python, poetry, uv, go, cargo-workspace, cmake, cmake-ninja, and generic
- Lists commands and descriptions with `mbr list`
- Shows resolved config with `mbr which`
- Inspects a command with `mbr show <name>`
- Supports `--workspace <path>` to run from a nested project root
- Runs multiple named commands concurrently with `mbr parallel <name>...`
- Supports pipeline commands with `steps = ["fmt", "lint", "test"]`
- Checks for missing commands and PATH issues with `mbr doctor`
- Supports `--dry-run` for execution commands
- Supports per-command `cwd` and `timeout`
- Uses the configured project root when provided
- Passes config environment variables to the child process
- Streams command output directly to the terminal
- Supports `extends` for derived commands that inherit arguments and environment by default
- Supports `args_mode = "replace"` and `env_mode = "replace"` for derived commands
- Supports profile overlays via `MBR_PROFILE`
- Supports `windows` and `unix` command overrides
- Supports `--safe` to reject shell-string commands
- Warns when the project has no explicit name
- Loads a project-root `.env` file
- Supports `retries` on commands
- Supports `workspace` to discover and run commands across projects
- Supports `package` to create local release archives
- Supports `completions` and `manpage` generation

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
