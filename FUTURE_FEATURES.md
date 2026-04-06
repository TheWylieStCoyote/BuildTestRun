# Future Features

## Config And Command Model

1. Pipeline commands
Let commands run named steps without shell chaining.

```toml
[commands.ci]
steps = ["fmt", "lint", "test"]
```

2. Per-command working directory
Useful for monorepos and mixed-language repos.

3. Per-command timeout
Prevents hung builds or tests in CI.

4. Profiles
Examples: `dev`, `release`, `ci`.

5. OS-specific overrides
Useful for Windows vs Unix differences.

## UX And Introspection

1. `btr show <command>`
Print the fully resolved command, cwd, env, and config source.

2. `btr list --verbose`
Show descriptions, command type, cwd, and env overrides.

3. `btr explain`
Explain why a command resolved the way it did.

4. Better JSON output
Keep machine-readable output stable for CI and editor integrations.

## Validation And Safety

1. `btr doctor --strict`
Exit non-zero on real problems.

2. `btr validate --strict`
Fail on missing conventional commands like `build`, `test`, and `run`.

3. Trust model
Warn before executing commands in unknown repos.

4. Safe mode
Allow only structured commands, not shell strings.

## Execution

1. Environment file support
Load `.env` or named env files for commands.

2. Command inheritance
Shared defaults plus per-command overrides.

3. Parallel execution
Useful for lint, docs, and format checks.

4. Retry support
Helpful for flaky tooling.

## Projects And Workspaces

1. Workspace mode
Run commands across multiple configured projects.

2. Config inheritance
Parent `.btr.toml` with project overrides.

3. Project discovery
List all configured projects under a repo.

## Release And Ecosystem

1. Release packaging
A first-class `release` command convention.

2. Installation helpers
Generate shell completions and man pages.

3. Template catalog expansion
Add templates for:

- pnpm
- yarn
- poetry
- uv
- cargo workspaces
- CMake + Ninja

## Best Next Features

1. `doctor --strict`
2. `btr show`
3. Per-command `cwd`
4. Pipeline commands
5. OS-specific overrides

## Suggested Priority

- Short term: `doctor --strict`, `btr show`, per-command `cwd`
- Medium term: pipelines, OS overrides, profiles
- Long term: workspace mode and config inheritance
