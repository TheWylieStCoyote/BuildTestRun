# Build and Test Flag Passthrough

## Problem

Users may want to pass build or test flags such as:

- `--release`
- `--target x86_64-unknown-linux-gnu`
- test filters
- runtime arguments

## Recommended Approach

Support argument passthrough from `btr` to the underlying command.

Example:

```bash
btr build -- --release
btr build -- --target wasm32-unknown-unknown
btr test -- my_test_name
btr run -- --port 8080
```

This keeps the hidden project config simple:

```toml
[commands]
build = "cargo build"
test = "cargo test"
run = "cargo run"
```

## Expected Behavior

- `btr build -- ...` forwards args to the configured build command.
- `btr test -- ...` forwards args to the configured test command.
- `btr run -- ...` forwards args to the configured run command.

## Why This Works Well

1. It avoids adding lots of named config variants.
2. It works across many ecosystems.
3. It solves build flags, test filters, and runtime arguments in one design.

## Implementation Note

The safest long-term design is a structured command model, but shell-string commands with passthrough are a reasonable first step.

Example future shape:

```toml
[commands.build]
program = "cargo"
args = ["build"]
```

## Recommendation

Start with passthrough for `build`, `test`, and `run`, then move toward structured commands later for better correctness and quoting behavior.
