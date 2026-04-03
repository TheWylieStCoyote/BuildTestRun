# MakeBuildRun Examples

## Rust Project

```toml
[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"] }
```

Usage:

```bash
mbr build -- --release
mbr test -- my_test_name
mbr exec lint
mbr validate
mbr init
mbr list
mbr which
mbr doctor
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
run = { program = "./build/native-app", args = [] }
```

## Common Pattern

Each project keeps a hidden `.mbr.toml` file at its root. The CLI discovers it automatically when you run commands from the project root or from any nested directory.

```bash
mbr build
mbr test
mbr run
mbr exec ci
```
