# MakeBuildRun Project Specification

## Overview

MakeBuildRun is a Rust command line tool that standardizes common project actions for arbitrary projects:

```bash
mbr build
mbr test
mbr run
mbr exec <name>
mbr validate
mbr init
mbr templates
mbr list
mbr which
mbr doctor
mbr show <name>
mbr --workspace <path>
mbr parallel <name>...
mbr fmt
mbr clean
mbr ci
```

The tool reads project-specific instructions from a hidden configuration file stored in each target project. The config tells the CLI how to build, test, and run that project without the CLI needing built-in support for a specific language or framework.

## Goal

Provide a single, predictable command interface that works across many ecosystems while keeping project-specific behavior in the project itself.

## Core Behavior

When a user runs `mbr build`, `mbr test`, `mbr run`, or `mbr exec <name>`, the CLI should:

1. Find the hidden config file in the current directory or one of its parents.
2. Merge any parent `.mbr.toml` files with the nearest child overrides.
3. Parse and validate the config.
4. Resolve the command for the requested action.
5. Execute that command in the configured project directory.
6. Stream stdout and stderr directly to the terminal.
7. Exit with the same status code as the underlying command.

`mbr validate` should parse and validate the config without executing anything.
`mbr validate --strict` should fail on missing conventional commands, missing tools, missing env files, and placeholder `run` commands.
`mbr init` should create a starter `.mbr.toml` in the current directory, with templates for common ecosystems.
Template variants should include rust, node, pnpm, yarn, bun, deno, nextjs, vite, turbo, nx, python, django, fastapi, flask, poetry, hatch, pixi, uv, go, cargo-workspace, java-gradle, java-maven, kotlin-gradle, dotnet, php-composer, ruby-bundler, rails, laravel, terraform, helm, docker-compose, cmake, cmake-ninja, and generic.
`mbr init --interactive` should prompt for project name, root, and template.
`mbr init --interactive` may also prompt for template-specific optional commands and safe structured-only mode.
`mbr init --detect` should infer a starter template from common marker files like `Cargo.toml`, `package.json`, `pyproject.toml`, and `CMakeLists.txt`.
`mbr init --list-templates` and `mbr templates` should list starter templates with descriptions.
`mbr init --template-file <path>` should render a custom starter template from a file or a directory containing a template file.
Rendered init templates should be validated before writing.
`mbr list` should print available command names and optional descriptions.
`mbr which` should show the resolved config path and project root.
`mbr doctor` should report missing commands and PATH issues.
`mbr doctor --strict` should exit non-zero when warnings exist.
`mbr show <name>` should display the resolved command, cwd, timeout, and description.
`mbr --workspace <path>` should resolve the project starting from the given directory.
`mbr parallel <name>...` should run multiple named commands concurrently.
Pipeline commands should support `steps = ["fmt", "lint", "test"]` and run each named step in order.
`extends` on a command should inherit base fields and append arguments by default. Use `args_mode = "replace"` to replace inherited args, and `env_mode = "replace"` to replace inherited env.
Set `MBR_PROFILE=<name>` to apply `[profiles.<name>]` overlays.
`--profile <name>` should select a profile explicitly and override `MBR_PROFILE`.
`env_file = ".env.ci"` should load named environment files from the project root.
Commands may include `windows = { ... }` and `unix = { ... }` override tables for platform-specific differences.
`--safe` should reject shell-string commands.
If `[project].name` is missing, execution should warn that command trust is lower.
If a project-root `.env` file exists, its values should be loaded before execution.
Commands may define `retries` to retry failed runs.
`mbr workspace --list` should list discovered projects, `mbr workspace --name <project> --list` should filter by project name, and `mbr workspace --name <project> <name>` should run a named command in each matching discovered project.
`mbr package` should archive the configured project root into a local tarball or zip file.
`mbr release` should run build and test before creating a package archive.
`mbr completions <shell>` should print a shell completion script.
`mbr manpage` should print the command manpage to stdout.
`install.sh` should be able to write generated completions and the manpage into user-supplied directories.

## Config File

Recommended filename: `.mbr.toml`

Example:

```toml
[project]
name = "example-app"
root = "."

[env]
RUST_LOG = "info"

[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"] }
```

## Config Semantics

- `project.name`: Optional human-readable project name.
- `project.root`: Optional working directory for command execution.
- `env`: Optional environment variables applied to every command.
- `commands.build`: Command executed by `mbr build`.
- `commands.test`: Command executed by `mbr test`.
- `commands.run`: Command executed by `mbr run`.
- `commands.<name>`: Additional named commands executed by `mbr exec <name>`.
- `commands.<name>.steps`: Sequential named steps for pipeline commands.
- `profiles.<name>`: Optional environment and command overlays activated by `MBR_PROFILE`.
- `commands.fmt`, `commands.clean`, `commands.ci`: Common convenience commands.
- `cwd`: Optional per-command working directory relative to the project root.
- `timeout`: Optional per-command timeout in seconds.

## CLI Commands

Minimum viable commands:

- `mbr build`
- `mbr test`
- `mbr run`
- `mbr exec <name>`
- `mbr validate`
- `mbr init`
- `mbr list`
- `mbr which`
- `mbr doctor`
- `mbr fmt`
- `mbr clean`
- `mbr ci`
- `mbr --help`
- `mbr --version`

Recommended future commands:

- `mbr validate`
- `mbr init`
- `mbr doctor`

## Error Handling

The CLI should produce clear errors for:

- Missing config file
- Invalid TOML
- Missing `[commands]` section
- Missing command definition
- Invalid project root path
- Command execution failure
- Non-zero exit codes from the child process

## Rust Architecture

Suggested modules:

- `src/main.rs`: entry point
- `src/cli.rs`: argument parsing
- `src/config.rs`: TOML config parsing and validation
- `src/discovery.rs`: config file lookup
- `src/runner.rs`: process execution
- `src/error.rs`: shared error types

## Testing Strategy

Testing should cover unit tests, integration tests, and manual smoke tests.

### Unit Tests

Use unit tests for small pieces of logic:

- Parse valid TOML into config structs
- Reject malformed TOML
- Reject empty command groups
- Reject missing required command entries
- Resolve config file location from nested directories
- Validate working directory resolution

### Integration Tests

Use `assert_cmd` and `tempfile` to test the CLI end to end:

- `mbr build` succeeds with a valid config
- `mbr test` fails cleanly when the configured command fails
- `mbr run` reports missing config clearly
- `mbr exec <name>` runs named commands
- extra arguments are forwarded after `--`
- Running from a subdirectory still finds the root config
- Environment variables from the config reach the child process

### Manual Smoke Tests

Test the CLI against real sample projects:

- Rust project with `cargo`
- Node project with `npm`
- Python project with `pytest` or a simple script

Verify that each project responds correctly to `build`, `test`, and `run`.

## Acceptance Criteria

The project is ready when:

1. `mbr build`, `mbr test`, and `mbr run` work for any project with a valid config.
2. Config discovery works from subdirectories.
3. Errors are clear and actionable.
4. Exit codes are preserved.
5. Tests cover success and failure paths.

## Recommended First Implementation

1. Initialize the Rust crate.
2. Add CLI parsing.
3. Add config parsing and discovery.
4. Add command execution.
5. Add unit and integration tests.
