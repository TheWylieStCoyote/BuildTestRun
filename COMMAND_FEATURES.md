# Command Features

## Supported Commands

The CLI should support project-defined commands beyond `build`, `test`, and `run`.

Recommended built-in command names:

- `build`
- `test`
- `run`
- `fmt`
- `clean`
- `ci`
- any custom command via `mbr exec <name>`

## Typical Usage

```bash
mbr build
mbr test
mbr run
mbr fmt
mbr clean
mbr ci
mbr exec lint
```

## Suggested Semantics

- `mbr fmt` should format the project using the project-defined formatter.
- `mbr clean` should remove generated build artifacts.
- `mbr ci` should run the project's CI workflow.

## Example Node Config

```toml
[commands]
build = { program = "npm", args = ["run", "build"] }
test = { program = "npm", args = ["test"] }
run = { program = "npm", args = ["start"] }
fmt = { program = "npm", args = ["run", "format"] }
clean = { program = "npm", args = ["run", "clean"] }
ci = { program = "npm", args = ["run", "ci"] }
```

## Recommendation

Keep these as thin aliases over project-configured commands so the CLI stays language-agnostic.
