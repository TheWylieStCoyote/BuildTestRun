# MakeBuildRun Examples

## Rust Project

```toml
[project]
name = "api-server"
root = "."

[commands]
build = "cargo build"
test = "cargo test"
run = "cargo run"
```

Usage:

```bash
mbr build
mbr test
mbr run
```

## Node Project

```toml
[project]
name = "frontend"

[commands]
build = "npm run build"
test = "npm test"
run = "npm start"
```

## Python Project

```toml
[project]
name = "worker"

[env]
PYTHONUNBUFFERED = "1"

[commands]
build = "python -m build"
test = "pytest"
run = "python main.py"
```

## Go Project With Custom Root

```toml
[project]
name = "go-service"
root = "services/api"

[commands]
build = "go build ./..."
test = "go test ./..."
run = "go run ."
```

## CMake Project

```toml
[project]
name = "native-app"
root = "."

[commands]
build = "cmake -S . -B build && cmake --build build"
test = "ctest --test-dir build"
run = "./build/native-app"
```

If your executable name differs, update the `run` command to match the output binary.

## Common Pattern

Each project keeps a hidden `.mbr.toml` file at its root. The CLI discovers it automatically when you run commands from the project root or from any nested directory.

```bash
mbr build
mbr test
mbr run
```
