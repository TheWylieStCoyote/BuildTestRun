# Feature Opportunities

## Highest Impact

1. [x] `init --import`
Generate `.mbr.toml` from existing project files like `Cargo.toml`, `package.json`, `pyproject.toml`, `Makefile`, and `justfile`.

2. [x] Workspace controls
Add `workspace --jobs <n>`, `--fail-fast`, `--keep-going`, and ordered execution options.

3. [x] `watch` mode
Add `mbr watch build`, `mbr watch test`, and `mbr watch workspace ...` for tight feedback loops.

## Strong Follow-Ups

4. [x] Explicit requirements in config
Let projects declare required tools, files, and env vars so `doctor` can validate them directly.

5. [x] Trust model improvements
Add an explicit trust workflow for shell-based commands and untrusted repositories.

6. [x] JSON event streams
Keep stable final JSON envelopes, but add optional streaming events for long-running workspace and parallel runs.

## Nice To Have

- [x] Project tags or selectors for workspace filtering
- [x] A `show --tree` or graph view for inheritance and pipelines
- [x] `--log-dir` for saved command output
- Editor-friendly schema or completion support for `.mbr.toml`
- Named command parameters beyond raw passthrough args

## Avoid For Now

- Plugins
- Remote templates
- Complex config conditionals
- Heavy caching/orchestration that moves the tool toward Nx or Bazel territory
