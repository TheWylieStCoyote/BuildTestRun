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
- any custom command via `btr exec <name>`

## Typical Usage

```bash
btr build
btr test
btr run
btr fmt
btr clean
btr ci
btr exec lint
```

## Suggested Semantics

- `btr fmt` should format the project using the project-defined formatter.
- `btr clean` should remove generated build artifacts.
- `btr ci` should run the project's CI workflow.

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
