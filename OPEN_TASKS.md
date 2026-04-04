# Open Tasks

## Highest Priority Missing Features

- [x] Pipeline commands (`steps = ["fmt", "lint", "test"]`)
- [x] OS-specific overrides
- [x] Profiles (`dev`, `release`, `ci`)
- [x] Watch/dev mode via first-class `mbr dev`

## UX And Introspection

- [x] `mbr list --verbose`
- [x] `mbr explain`
- [x] Better JSON output stability

## Validation And Safety

- [x] Safe mode for structured-only commands
- [x] Trust model warnings for unknown repos
- [x] Deeper validation for missing tools, placeholder `run` commands, and env files

## Execution

- [x] Environment file support
- [x] Retry support

## Projects And Workspaces

- [x] Workspace mode across multiple projects
- [x] Project discovery for configured repos
- [x] Workspace name filtering

## Release And Ecosystem

- [x] Release packaging
- [x] Shell completions and man pages
- [x] Template catalog expansion for pnpm, yarn, poetry, uv, cargo workspaces, and CMake + Ninja
- [x] Template metadata and listing
- [x] Template snapshots and validation coverage
- [x] Per-template optional prompts
- [x] Machine-readable output contracts
- [x] Install-time completion and manpage helpers
