# Init Enhancements Checklist

## Implemented Templates

- [x] `rust` - Cargo build/test/run/fmt/clean/lint with a structured `ci` pipeline.
- [x] `node` - npm scripts for build/test/run/dev/fmt/lint/typecheck/clean with pipeline `ci`.
- [x] `pnpm` - pnpm script-based app template with build/test/run/dev/fmt/lint/typecheck/clean/ci.
- [x] `yarn` - Yarn script-based app template with build/test/run/dev/fmt/lint/typecheck/clean/ci.
- [x] `bun` - Bun-native scripts plus `bunx` for formatting and TypeScript checks.
- [x] `deno` - Deno tasks plus `fmt`, `lint`, and `check` from Deno tooling.
- [x] `nextjs` - Next.js defaults with build/test/run/dev/lint/typecheck/fmt/clean/ci.
- [x] `vite` - Vite frontend defaults with preview-based run mode and JS checks.
- [x] `turbo` - Turborepo workspace commands for build/test/dev/lint/typecheck/fmt/ci.
- [x] `nx` - Nx monorepo commands with workspace-wide targets and cache reset cleanup.
- [x] `python` - Plain Python packaging and test workflow with `ruff` and `mypy`-style checks.
- [x] `django` - Django `manage.py`-centric template with test/run/check and ruff-based formatting.
- [x] `fastapi` - ASGI template using `uvicorn`, `pytest`, `ruff`, and `mypy`.
- [x] `flask` - Flask app template with `flask run --debug`, tests, and Python tooling.
- [x] `poetry` - Poetry-managed project template with `poetry run` wrappers for checks and execution.
- [x] `hatch` - Hatch-managed workflow with `hatch build`, `hatch test`, and tool-backed checks.
- [x] `pixi` - Task-oriented Pixi template using `pixi run` for build/test/dev/lint/typecheck.
- [x] `uv` - `uv`-driven Python workflow using `uv build` and `uv run` for tools.
- [x] `go` - Go module defaults with `build`, `test`, `run`, `fmt`, `lint`, `check`, and `ci`.
- [x] `cargo-workspace` - Cargo workspace template with workspace-wide build/test/lint/check/ci commands.
- [x] `java-gradle` - Gradle wrapper commands with Windows/Unix overrides and build/test/run/check/ci.
- [x] `java-maven` - Maven lifecycle commands with build/test/run/check/ci conventions.
- [x] `kotlin-gradle` - Kotlin Gradle wrapper template aligned with Gradle-based JVM conventions.
- [x] `dotnet` - .NET CLI workflow with build/test/run/dev/format/check/clean/ci.
- [x] `php-composer` - Composer-centric template using composer scripts for build/test/run/lint/check/ci.
- [x] `ruby-bundler` - Bundler/Rake/RSpec/RuboCop-based workflow for Ruby projects.
- [x] `rails` - Rails app template with bin/rails defaults and RuboCop-based formatting/linting.
- [x] `laravel` - Laravel template using artisan, Pint, and standard framework commands.
- [x] `terraform` - Terraform-oriented template with fmt/validate/plan/apply-oriented commands.
- [x] `helm` - Helm chart template with lint, package, template, and clean commands.
- [x] `docker-compose` - Docker Compose template with build/up/down/clean and config validation.
- [x] `cmake` - CMake configure/build/test/clean workflow with a placeholder executable target.
- [x] `cmake-ninja` - CMake + Ninja variant with the same workflow and Ninja generator defaults.
- [x] `generic` - Minimal placeholder template for manual customization.

## Implemented Improvements

- [x] Replace shell-string CI entries with structured pipelines where possible.
- [x] Add more commands like `lint`, `check`, `dev`, `docs`, and `typecheck` where they make sense.
- [x] Make `run` less placeholder-heavy in `cmake`.
- [x] Prefer structured commands over inline shell for cleanup and CI tasks.
- [x] Improve descriptions so `list --verbose` and `show` are more useful.
- [x] Make Python templates reflect toolchain style more clearly.

## Implemented Init UX

- [x] Add `btr init --interactive`.
- [x] Prompt for project name.
- [x] Prompt for template.
- [x] Prompt for project root.
- [x] Prompt for optional commands.
- [x] Prompt for safe structured-only mode.

## Implemented Custom Templates

- [x] Add support for `btr init --template-file <path>`.
- [x] Render placeholders before writing `.btr.toml`.
- [x] Validate rendered TOML before writing.
- [x] Add directory-based custom templates.

## Planned Follow-Up

- [x] Add more template-specific optional commands.
- [x] Improve `cmake` run commands.
- [x] Add interactive toggles for optional commands.
- [x] Add post-render validation for custom templates.
