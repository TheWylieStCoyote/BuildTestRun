# Init Enhancements

## Additional Templates

- `bun`
- `deno`
- `nextjs`
- `vite`
- `turbo`
- `nx`
- `django`
- `fastapi`
- `flask`
- `hatch`
- `pixi`
- `java-gradle`
- `java-maven`
- `kotlin-gradle`
- `dotnet`
- `php-composer`
- `ruby-bundler`
- `rails`
- `laravel`
- `terraform`
- `helm`
- `docker-compose`

## Existing Template Improvements

- Replace shell-string CI entries with structured pipelines where possible
- Add more commands like `lint`, `check`, `docs`, `dev`, and `typecheck`
- Make `run` less placeholder-heavy in `cmake` and `cargo-workspace`
- Prefer structured commands over inline shell for cleanup and CI tasks
- Improve descriptions so `list --verbose` and `show` are more useful
- Make Python templates reflect toolchain style more clearly

## Init UX

- Add `mbr init --interactive`
- Prompt for project name
- Prompt for template
- Prompt for project root
- Prompt for optional commands
- Prompt for safe structured-only mode

## Custom Templates

- Add support for `mbr init --template-file <path>`
- Render placeholders before writing `.mbr.toml`
- Validate rendered TOML before writing

## Open Decisions

- Prompt implementation: standard stdin/stdout or a prompt crate
- Custom template support: single file first or file plus directory support
