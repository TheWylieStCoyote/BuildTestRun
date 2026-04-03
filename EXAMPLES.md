# MakeBuildRun Examples

## Rust Project

```toml
[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"] }
docs = { program = "cargo", args = ["doc"], cwd = "docs" }
check = { program = "cargo", args = ["check"], timeout = 60 }
```

Usage:

```bash
mbr build -- --release
mbr test -- my_test_name
mbr exec lint
mbr validate --strict
mbr init --template node
mbr init --interactive
mbr init --template-file custom-template.toml
mbr list
mbr which
mbr doctor --strict
mbr show build
mbr --workspace web build
mbr parallel fmt lint test
```

## Node Project

```toml
[commands]
build = { program = "npm", args = ["run", "build"] }
test = { program = "npm", args = ["test"] }
run = { program = "npm", args = ["start"] }
fmt = { program = "npm", args = ["run", "format"] }
clean = { program = "npm", args = ["run", "clean"] }
ci = { program = "npm", args = ["run", "ci"] }
dev = { program = "npm", args = ["run", "dev"], cwd = "web" }
```

## Python Project

```toml
[env]
PYTHONUNBUFFERED = "1"

[commands]
build = { program = "python", args = ["-m", "build"] }
test = { program = "pytest", args = [] }
run = { program = "python", args = ["main.py"] }
analyze = { program = "bandit", args = ["-r", "."] }
```

## Go Project With Custom Root

```toml
[project]
root = "services/api"

[commands]
build = { program = "go", args = ["build", "./..."] }
test = { program = "go", args = ["test", "./..."] }
run = { program = "go", args = ["run", "."] }
```

## CMake Project

```toml
[commands]
build = { program = "cmake", args = ["-S", ".", "-B", "build"] }
build_release = { program = "cmake", args = ["--build", "build", "--config", "Release"] }
test = { program = "ctest", args = ["--test-dir", "build"] }
run = { program = "cmake", args = ["--build", "build", "--target", "run"], description = "Replace with your executable target" }
```

## Common Pattern

Each project keeps a hidden `.mbr.toml` file at its root. The CLI discovers it automatically when you run commands from the project root or from any nested directory.
Nested projects can add their own `.mbr.toml` files to override or extend parent commands.

Command tables can also inherit from other commands with `extends = "name"`.

```toml
[commands]
build = { program = "cargo", args = ["build", "--locked"] }
release = { extends = "build", args_mode = "replace", args = ["build", "--release"] }
```

Use `args_mode = "replace"` when you want to swap out inherited flags, and `env_mode = "replace"` when you want a child command to ignore inherited env vars.

Profiles can switch project overlays without editing the base file:

```toml
[profiles.ci]
env = { RUST_LOG = "warn" }

[profiles.ci.commands]
ci = { steps = ["fmt", "lint", "test"] }
```

Platform-specific overrides let one config work on Windows and Unix:

```toml
[commands]
build = { program = "cargo", args = ["build"], windows = { program = "cmd", args = ["/C", "echo build"] } }
```

Safe mode rejects shell-string commands:

```bash
mbr --safe build
```

Retry failed commands when necessary:

```toml
[commands]
build = { program = "cargo", args = ["build"], retries = 1 }
```

Workspace mode runs a command in each discovered project:

```bash
mbr workspace --list
mbr workspace build
```

Package a release archive locally:

```bash
mbr package --output demo.tar.gz
```

Generate shell completions or a manpage:

```bash
mbr completions bash
mbr manpage
```

Interactive init:

```bash
mbr init --interactive
```

Custom template file:

```bash
mbr init --template-file custom-template.toml
```

Pipeline commands avoid shell chaining:

```toml
[commands]
fmt = { program = "cargo", args = ["fmt"] }
lint = { program = "cargo", args = ["clippy"] }
test = { program = "cargo", args = ["test"] }
ci = { steps = ["fmt", "lint", "test"] }
```

```bash
mbr build
mbr test
mbr run
mbr exec ci
```
