# Feature Roadmap

## Goals

MakeBuildRun should support more than `build`, `test`, and `run`. The next useful capabilities are:

- argument passthrough
- named commands
- linting
- code analysis
- CI workflows

## Recommended Direction

Use a generic command registry in `.btr.toml` so projects can define commands like:

```toml
[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
lint = { program = "cargo", args = ["clippy", "--all-targets", "--all-features", "--", "-D", "warnings"] }
analyze = { program = "cargo", args = ["audit"] }
ci = { program = "cargo", args = ["fmt", "--check"] }
```

## CLI Shape

Recommended commands:

```bash
btr build -- --release
btr test -- my_test_name
btr run -- --port 8080
btr exec lint
btr exec analyze
btr exec ci
```

## Why This Helps

1. `build`, `test`, and `run` remain simple defaults.
2. `exec` handles arbitrary project commands.
3. Linting and code analysis fit naturally as named commands.
4. CI can be defined per project instead of hardcoded in the CLI.

## Suggested Next Steps

1. Add structured command definitions.
2. Support passthrough arguments.
3. Add `btr exec <name>`.
4. Add examples for lint, analyze, and CI.
5. Add tests for passthrough and named commands.
