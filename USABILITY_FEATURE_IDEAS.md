# Usability Feature Ideas

## Best Next Features

1. [x] `init --detect`
Detect Rust, Node, Python, and CMake projects automatically from files like `Cargo.toml`, `package.json`, `pyproject.toml`, and `CMakeLists.txt`.

2. [x] `workspace --changed-only`
Only run commands for projects touched in the current git diff.

3. [x] Prefixed workspace and parallel output
Prefix output with the project or command name so multi-project runs stay readable.

4. Better failure summaries
Summarize failed commands with exit code, project name, and duration.

5. Source-aware `show` / `explain`
Show where each resolved value came from: base config, child config, profile, or platform override.

6. Consistent JSON envelopes
Use stable `status`, `count`, `project`, `command`, and `warnings` fields across commands.

7. More actionable `doctor`
Suggest likely fixes when PATH tools or env files are missing.

8. `init --print`
Render a starter config to stdout instead of writing a file.

9. Better provenance in `which`
Show config chain and selected profile clearly.

10. Template detection plus prompts
Start from a detected ecosystem, then ask only the most useful follow-up questions.

## What Would Make It Feel Really Usable

- Zero-config adoption through detection.
- Clear, stable output for humans and machines.
- Strong introspection for resolved config and execution choices.
- Monorepo-friendly workspace filtering and summaries.
- Fast onboarding through templates, doctor checks, and install helpers.

## Guardrails

- Avoid arbitrary config conditionals.
- Avoid plugin systems too early.
- Avoid remote template execution without trust checks.
- Keep the command model simple and predictable.
