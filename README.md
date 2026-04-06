# Build Test Run

`btr` is a Rust CLI for running project-defined commands from a hidden project config file.

## What It Does

Each project contains a `.btr.toml` file that defines how to build, test, run, lint, analyze, and execute other named commands. The CLI discovers that file automatically and executes the configured command for the action you choose.

## Commands

```bash
btr build
btr test
btr run
btr dev
btr exec lint
btr validate --strict
btr --profile dev build
btr init --template node
btr init --interactive
btr init --detect
btr init --import
btr init --list-templates
btr init --template-file custom-template.toml
btr templates
btr list
btr list --verbose
btr which
btr schema
btr doctor --strict
btr show build
btr show --tree build
btr explain build
btr --workspace workspace build
btr workspace --name api build
btr workspace --tag web build
btr workspace --changed-only build
btr workspace --jobs 4 build
btr workspace --fail-fast build
btr watch --once build
btr parallel fmt lint test
btr --log-dir logs build
btr workspace --list
btr workspace build
btr package --output demo.tar.gz
btr release --output demo.tar.gz
btr completions bash
btr manpage
btr fmt
btr clean
btr ci
```

## Config File

Create a `.btr.toml` file at the root of your project:

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

- Finds `.btr.toml` in the current directory or a parent directory
- Merges parent `.btr.toml` files with child overrides
- Runs the matching command for `build`, `test`, or `run`
- Runs the matching command for `build`, `test`, `run`, or `dev`
- Supports named commands via `btr exec <name>`
- Forwards extra arguments after `--`
- Validates config with `btr validate`
- Supports `--strict` for `validate` and `doctor`
- Strict validation checks PATH availability, env files, and placeholder `run` commands
- Generates a starter config with `btr init --template <name>`
- Supports interactive prompts with `btr init --interactive`
- Detects common project types with `btr init --detect`
- Imports common project files with `btr init --import`
- `btr init --detect --interactive` uses the detected template as the default prompt
- Prints starter configs with `btr init --print`
- Interactive init can add template-specific optional commands and safe structured-only mode
- Lists starter templates with `btr templates` or `btr init --list-templates`
- Prints a JSON schema for `.btr.toml` with `btr schema`
- Supports custom template files or directories with `btr init --template-file <path>`
- Interactive init can add optional command stubs and enable safe structured-only mode
- Starter templates include rust, node, pnpm, yarn, bun, deno, nextjs, vite, turbo, nx, python, django, fastapi, flask, poetry, hatch, pixi, uv, go, cargo-workspace, java-gradle, java-maven, kotlin-gradle, dotnet, php-composer, ruby-bundler, rails, laravel, terraform, helm, docker-compose, cmake, cmake-ninja, and generic
- Starter templates now include short descriptions in the template catalog
- Lists commands and descriptions with `btr list`
- Shows resolved config with `btr which`, including config chain and selected profile
- Inspects a command with `btr show <name>` and shows source provenance
- Shows command inheritance and pipeline trees with `btr show --tree <name>`
- Explains a command with `btr explain <name>` and shows source provenance
- JSON output now uses a stable envelope with `status` and `command`
- Supports `--workspace <path>` to run from a nested project root
- Runs multiple named commands concurrently with `btr parallel <name>...`
- Supports workspace concurrency with `btr workspace --jobs <n>`
- Supports workspace failure policies with `--fail-fast` and `--keep-going`
- Supports workspace ordering with `--order name`
- Supports workspace filtering with project tags via `--tag <tag>`
- Supports `btr watch` for repeated execution on file changes
- Supports `[requirements]` for required tools, files, and env vars
- Supports `[trust].shell_commands` to explicitly allow shell-based commands
- Supports `--json-events` for streaming orchestration progress to stderr
- Supports `--log-dir` for saving command output to files
- Supports pipeline commands with `steps = ["fmt", "lint", "test"]`
- Prefixes workspace and parallel output with the project or command name
- Prints failure summaries with exit code, target, and duration
- Prints end-of-run summaries for run, workspace, parallel, dry-run, and release
- Checks for missing commands and PATH issues with `btr doctor`
- `btr doctor` suggests fixes for missing PATH tools and env files
- `btr doctor --fix` creates missing configured env files when possible
- `btr show --source` prints an explicit provenance trace for config resolution
- Supports `--dry-run` for execution commands
- Supports per-command `cwd` and `timeout`
- Uses the configured project root when provided
- Passes config environment variables to the child process
- Streams command output directly to the terminal
- Supports `extends` for derived commands that inherit arguments and environment by default
- Supports `args_mode = "replace"` and `env_mode = "replace"` for derived commands
- Supports profile overlays via `BTR_PROFILE`
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
./install.sh --install-completions /tmp/btr-completions --install-manpage /tmp/btr-manpages
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
./install.sh --root /tmp/btr --debug --force
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

- Lint: `btr exec lint`
- Analysis: `btr exec analyze`
- CI: `btr exec ci`
- Format: `btr fmt`
- Clean: `btr clean`
- CI alias: `btr ci`
