# New Roadmap

## Stage 1

1. Named command parameters
Allow commands to declare named inputs instead of only raw passthrough args.

2. Editor integration polish
Document editor wiring for `btr schema` and optionally make schema installable.

3. Examples and cookbook refresh
Expand `EXAMPLES.md` with profiles, requirements, trust, tags, watch, events, logs, and schema setup.

## Stage 2

1. Richer workspace selectors
Extend tags/selectors with include/exclude patterns and other filtering options.

2. Watch include/exclude controls
Add include/exclude glob controls and configurable ignores.

3. `--log-dir` manifest/index output
Write a small manifest with command, project, timestamps, log paths, and exit code.

## Stage 3

1. Explicit trust workflow command
Add a `trust` command and better bootstrap guidance for shell-command trust.

2. Doctor expansion
Teach `doctor` to validate more real-world issues and suggest fixes.

3. JSON summary refinements
Add richer aggregate summaries and log references for automation.
