# MakeBuildRun

`mbr` is a Rust CLI for running project-defined commands from a hidden project config file.

## What It Does

Each project contains a `.mbr.toml` file that defines how to build, test, run, lint, analyze, and execute other named commands. The CLI discovers that file automatically and executes the configured command for the action you choose.

## Commands

```bash
mbr build
mbr test
mbr run
mbr dev
mbr exec lint
mbr validate --strict
mbr --profile dev build
mbr init --template node
mbr init --interactive
mbr init --detect
mbr init --list-templates
mbr init --template-file custom-template.toml
mbr templates
mbr list
mbr list --verbose
mbr which
mbr doctor --strict
mbr show build
mbr explain build
mbr --workspace workspace build
mbr workspace --name api build
mbr workspace --changed-only build
mbr parallel fmt lint test
mbr workspace --list
mbr workspace build
mbr package --output demo.tar.gz
mbr release --output demo.tar.gz
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

env_file = ".env.ci"

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
- Runs the matching command for `build`, `test`, `run`, or `dev`
- Supports named commands via `mbr exec <name>`
- Forwards extra arguments after `--`
- Validates config with `mbr validate`
- Supports `--strict` for `validate` and `doctor`
- Strict validation checks PATH availability, env files, and placeholder `run` commands
- Generates a starter config with `mbr init --template <name>`
- Supports interactive prompts with `mbr init --interactive`
- Detects common project types with `mbr init --detect`
- `mbr init --detect --interactive` uses the detected template as the default prompt
- Prints starter configs with `mbr init --print`
- Interactive init can add template-specific optional commands and safe structured-only mode
- Lists starter templates with `mbr templates` or `mbr init --list-templates`
- Supports custom template files or directories with `mbr init --template-file <path>`
- Interactive init can add optional command stubs and enable safe structured-only mode
- Starter templates include rust, node, pnpm, yarn, bun, deno, nextjs, vite, turbo, nx, python, django, fastapi, flask, poetry, hatch, pixi, uv, go, cargo-workspace, java-gradle, java-maven, kotlin-gradle, dotnet, php-composer, ruby-bundler, rails, laravel, terraform, helm, docker-compose, cmake, cmake-ninja, and generic
- Starter templates now include short descriptions in the template catalog
- Lists commands and descriptions with `mbr list`
- Shows resolved config with `mbr which`, including config chain and selected profile
- Inspects a command with `mbr show <name>` and shows source provenance
- Explains a command with `mbr explain <name>` and shows source provenance
- JSON output now uses a stable envelope with `status` and `command`
- Supports `--workspace <path>` to run from a nested project root
- Runs multiple named commands concurrently with `mbr parallel <name>...`
- Supports pipeline commands with `steps = ["fmt", "lint", "test"]`
- Prefixes workspace and parallel output with the project or command name
- Prints failure summaries with exit code, target, and duration
- Checks for missing commands and PATH issues with `mbr doctor`
- `mbr doctor` suggests fixes for missing PATH tools and env files
- Supports `--dry-run` for execution commands
- Supports per-command `cwd` and `timeout`
- Uses the configured project root when provided
- Passes config environment variables to the child process
- Streams command output directly to the terminal
- Supports `extends` for derived commands that inherit arguments and environment by default
- Supports `args_mode = "replace"` and `env_mode = "replace"` for derived commands
- Supports profile overlays via `MBR_PROFILE`
- Supports `--profile <name>` to select a profile explicitly
- Supports `env_file = ".env.ci"` for named env files
- Supports `windows` and `unix` command overrides
- Supports `--safe` to reject shell-string commands
- Warns when the project has no explicit name
- Loads a project-root `.env` file
- Supports `retries` on commands
- Supports `workspace` to discover and run commands across projects
- Supports `workspace --name <project>` to filter discovered projects by project name
- Supports `workspace --changed-only` to limit execution to changed projects
- Supports `workspace --changed-only --since <ref>` to choose the git base
- Supports `package` to create local release archives
- Supports `release` to run build/test and then package
- Supports `completions` and `manpage` generation

## Development

Build and test the project with:

```bash
cargo test
```

Install the CLI locally with:

```bash
./install.sh
./install.sh --install-completions /tmp/mbr-completions --install-manpage /tmp/mbr-manpages
```

Installer options:

- `--root <dir>` to install into a custom Cargo root
- `--debug` to install a debug build
- `--force` to reinstall an existing binary
- `--no-lock` to skip lockfile enforcement
- `--check` to verify prerequisites only
- `--install-completions <dir>` to write shell completions
- `--install-manpage <dir>` to write the manpage

```bash
./install.sh --root /tmp/mbr --debug --force
./install.sh --check
./install.sh --help
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
