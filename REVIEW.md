# Code Review Notes

## Findings

1. `project.shell` is not implemented safely for arbitrary shells.
The runner always uses `-c` on Unix and `/C` on Windows, which only works for some shells. This makes the shell override misleading and likely to fail for valid configurations.

2. Missing config validation is being reported as parser failures.
The `commands` section is required during deserialization, so missing command definitions do not surface as clear validation errors.

3. The test suite does not cover several core behaviors.
There are no tests for missing command keys, non-zero exit propagation, env injection, or custom root execution.

4. Integration tests are Unix-biased.
The use of `printf` in test command bodies is not a good cross-platform choice.

5. The docs overstate current behavior.
The README and spec describe validation and error handling more completely than the implementation currently provides.

## Recommendations

1. Remove `project.shell` for now, or model it with explicit shell flags.
2. Add a post-parse validation layer for config semantics.
3. Expand integration coverage for the missing behavior.
4. Make tests platform-aware.
5. Align docs with the current implementation or finish the missing features.

## Suggested Next Features

1. `mbr validate`
2. `mbr init`
3. Named custom commands with `mbr exec <name>`
4. Structured command definitions instead of shell strings
5. Better runtime output for config path, working directory, and command summary

## Priority Order

1. Fix or remove `project.shell`
2. Add validation and clearer errors
3. Add missing tests
4. Add `mbr validate`
5. Add `mbr init`
6. Consider structured commands
