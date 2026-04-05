# Next Feature Ideas

## High-Value Additions

1. `mbr workspace --changed-only --since <ref>`
Let users choose the git base instead of only the default diff.

2. Command summaries
Print a short end-of-run summary: what ran, what failed, and how long it took.

3. `mbr doctor --fix`
Offer safe, obvious fixes for missing tools or missing config bits.

4. `mbr show --source`
Make provenance for inherited values and overrides even more explicit.

5. Finish JSON envelopes everywhere
Keep `--json` consistent across the rest of the CLI surface.

## What Makes It Usable

- Zero-config start.
- Predictable output.
- Great error messages.
- Clear config provenance.
- Monorepo-friendly workspace commands.
- Strong defaults with few flags.

## Guardrails

- Avoid plugins for now.
- Avoid remote template execution.
- Avoid complex config conditionals.
- Avoid too many new subcommands before the core UX is polished.
