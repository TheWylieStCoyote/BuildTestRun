use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use serde_json::Value;
use std::{fs, path::Path, process::Command as ProcessCommand};
use tempfile::TempDir;

fn write_config(dir: &Path, body: &str) {
    fs::write(dir.join(".mbr.toml"), body).expect("write config");
}

fn mkdir(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(&path).expect("create dir");
    path
}

fn run_git(dir: &Path, args: &[&str]) {
    let status = ProcessCommand::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {:?} failed", args);
}

fn print_command_spec(output: &str) -> String {
    if cfg!(windows) {
        format!(r#"{{ program = "cmd", args = ["/C", "echo {output}"] }}"#)
    } else {
        format!(r#"{{ program = "sh", args = ["-c", "printf {output}"] }}"#)
    }
}

fn arg_pair_command_spec() -> String {
    if cfg!(windows) {
        r#"{ program = "powershell", args = ["-NoProfile", "-Command", "& { param($a, $b) Write-Output ($a + '|' + $b) }"] }"#.to_string()
    } else {
        r#"{ program = "sh", args = ["-c", "printf '%s|%s' \"$1\" \"$2\"", "sh"] }"#.to_string()
    }
}

fn cwd_command_spec() -> String {
    if cfg!(windows) {
        r#"program = "powershell", args = ["-NoProfile", "-Command", "Get-Location | Select-Object -ExpandProperty Path"]"#.to_string()
    } else {
        r#"program = "sh", args = ["-c", "pwd"]"#.to_string()
    }
}

fn sleep_command_spec() -> String {
    if cfg!(windows) {
        r#"program = "powershell", args = ["-NoProfile", "-Command", "Start-Sleep -Seconds 2"]"#
            .to_string()
    } else {
        r#"program = "sh", args = ["-c", "sleep 2"]"#.to_string()
    }
}

fn sleep_and_print_command_spec(output: &str) -> String {
    if cfg!(windows) {
        format!(
            r#"{{ program = "powershell", args = ["-NoProfile", "-Command", "Start-Sleep -Seconds 2; Write-Output {output}"] }}"#
        )
    } else {
        format!(r#"{{ program = "sh", args = ["-c", "sleep 2; printf {output}"] }}"#)
    }
}

fn failing_command_spec(output: &str, exit_code: i32) -> String {
    if cfg!(windows) {
        format!(
            r#"{{ program = "powershell", args = ["-NoProfile", "-Command", "Write-Output {output}; exit {exit_code}"] }}"#
        )
    } else {
        format!(r#"{{ program = "sh", args = ["-c", "printf {output}; exit {exit_code}"] }}"#)
    }
}

fn env_values_command_spec() -> String {
    if cfg!(windows) {
        r#"program = "powershell", args = ["-NoProfile", "-Command", "Write-Output ($env:BASE + '|' + $env:KEEP + '|' + $env:CHILD)"]"#.to_string()
    } else {
        r#"program = "sh", args = ["-c", "printf '%s|%s|%s' \"$BASE\" \"$KEEP\" \"$CHILD\""]"#
            .to_string()
    }
}

fn single_env_command_spec(var: &str) -> String {
    if cfg!(windows) {
        format!(r#"{{ program = "cmd", args = ["/C", "echo %{var}%"] }}"#)
    } else {
        format!(r#"{{ program = "sh", args = ["-c", "printf '%s' \"${var}\""] }}"#)
    }
}

fn retrying_command_spec() -> String {
    if cfg!(windows) {
        r#"{ program = "cmd", args = ["/C", "if exist attempts.txt (type attempts.txt) else (echo retry-ok>attempts.txt & exit /b 1)"], retries = 1 }"#
            .to_string()
    } else {
        r#"{ program = "sh", args = ["-c", "if [ -f attempts.txt ]; then cat attempts.txt; else echo retry-ok > attempts.txt; exit 1; fi"], retries = 1 }"#
            .to_string()
    }
}

#[cfg(not(windows))]
fn archive_contains_file(archive_path: &Path, needle: &str) -> bool {
    let file = fs::File::open(archive_path).expect("open archive");
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().expect("entries");
    for entry in entries {
        let entry = entry.expect("entry");
        if entry
            .path()
            .expect("path")
            .to_string_lossy()
            .contains(needle)
        {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn archive_contains_file(archive_path: &Path, needle: &str) -> bool {
    let file = fs::File::open(archive_path).expect("open archive");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    for i in 0..archive.len() {
        let file = archive.by_index(i).expect("entry");
        if file.name().contains(needle) {
            return true;
        }
    }
    false
}

#[test]
fn build_runs_configured_command() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {}\ntest = {}\nrun = {}\n",
            print_command_spec("build-ok"),
            print_command_spec("test-ok"),
            print_command_spec("run-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains("build-ok"))
        .stderr(contains("[mbr] summary: command=build status=ok count=1"));
}

#[test]
fn dev_runs_configured_command() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\ndev = {}\n", print_command_spec("dev-ok")),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("dev")
        .assert()
        .success()
        .stdout(contains("dev-ok"));
}

#[test]
fn watch_once_runs_the_selected_command() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nbuild = {}\n", print_command_spec("watch-ok")),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["watch", "--once", "build"])
        .assert()
        .success()
        .stdout(contains("watch-ok"));
}

#[test]
fn build_forwards_extra_args() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nbuild = {}\n", arg_pair_command_spec()),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["build", "--", "--release", "--target"])
        .assert()
        .success()
        .stdout(contains("--release|--target"));
}

#[test]
fn executes_named_command() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nlint = {}\nbuild = {}\n",
            print_command_spec("lint-ok"),
            print_command_spec("build-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["exec", "lint"])
        .assert()
        .success()
        .stdout(contains("lint-ok"));
}

#[test]
fn fmt_clean_and_ci_run_project_commands() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nfmt = {}\nclean = {}\nci = {}\n",
            print_command_spec("fmt-ok"),
            print_command_spec("clean-ok"),
            print_command_spec("ci-ok")
        ),
    );

    for (cmd, expected) in [("fmt", "fmt-ok"), ("clean", "clean-ok"), ("ci", "ci-ok")] {
        Command::cargo_bin("mbr")
            .expect("binary")
            .current_dir(temp.path())
            .arg(cmd)
            .assert()
            .success()
            .stdout(contains(expected));
    }
}

#[test]
fn discovers_config_from_subdirectory() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nrun = {}\n", print_command_spec("run-ok")),
    );
    let nested = mkdir(temp.path(), "nested");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(nested)
        .arg("run")
        .assert()
        .success()
        .stdout(contains("run-ok"));
}

#[test]
fn reports_missing_config() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("test")
        .assert()
        .failure()
        .stderr(contains("no .mbr.toml found"));
}

#[test]
fn reports_missing_command_group() {
    let temp = TempDir::new().expect("temp dir");
    write_config(temp.path(), "[project]\nname = \"demo\"");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .failure()
        .stderr(contains("missing `[commands]` section"));
}

#[test]
fn validate_succeeds_for_valid_config() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nbuild = {}\n", print_command_spec("build-ok")),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("validate")
        .assert()
        .success()
        .stderr(contains("config valid"));
}

#[test]
fn validate_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nbuild = {}\n", print_command_spec("build-ok")),
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "validate"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "validate");
    assert!(value["warnings"].as_array().is_some());
}

#[test]
fn validate_strict_fails_for_missing_conventions() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["validate", "--strict"])
        .assert()
        .failure()
        .stderr(contains("missing test command"));
}

#[test]
fn validate_strict_reports_deeper_issues() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
env_file = ".env.missing"

[project]
name = "demo"

[commands]
build = { program = "definitely-not-on-path-12345" }
run = "echo run"
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["validate", "--strict"])
        .assert()
        .failure()
        .stderr(contains("env file `.env.missing` was not found"))
        .stderr(contains("was not found on PATH"))
        .stderr(contains("placeholder"));
}

#[test]
fn init_writes_starter_config() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success()
        .stderr(contains("wrote"));

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("[commands]"));
    assert!(contents.contains("program = \"cargo\""));
}

#[test]
fn init_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "init", "--print"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "init");
    assert!(value["rendered"].is_string());
    assert_eq!(value["printed"], true);
}

#[test]
fn init_prints_starter_config_without_writing() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join(".mbr.toml"),
        "[commands]\nbuild = \"echo existing\"\n",
    )
    .expect("seed config");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--print"])
        .assert()
        .success()
        .stdout(contains("[commands]"))
        .stdout(contains("program = \"cargo\""));

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("echo existing"));
}

#[test]
fn init_uses_requested_template() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--template", "node"])
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("program = \"npm\""));
    assert!(contents.contains("run = { program = \"npm\""));
}

#[test]
fn init_detects_template_from_project_markers() {
    let cases = [
        (
            "Cargo.toml",
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
            "program = \"cargo\"",
        ),
        (
            "package.json",
            "{\n  \"name\": \"demo\"\n}\n",
            "program = \"npm\"",
        ),
        (
            "pyproject.toml",
            "[project]\nname = \"demo\"\n",
            "program = \"python\"",
        ),
        (
            "CMakeLists.txt",
            "cmake_minimum_required(VERSION 3.20)\nproject(demo)\n",
            "program = \"cmake\"",
        ),
    ];

    for (file_name, contents, expected) in cases {
        let temp = TempDir::new().expect("temp dir");
        fs::write(temp.path().join(file_name), contents).expect("write marker");

        Command::cargo_bin("mbr")
            .expect("binary")
            .current_dir(temp.path())
            .args(["init", "--detect"])
            .assert()
            .success();

        let rendered = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
        assert!(
            rendered.contains(expected),
            "{file_name} should detect a matching template"
        );
    }
}

#[test]
fn init_imports_package_json_scripts() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join("package.json"),
        r#"{
  "name": "web-app",
  "scripts": {
    "build": "vite build",
    "start": "vite",
    "lint": "eslint ."
  }
}
"#,
    )
    .expect("write package.json");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--import"])
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("name = \"web-app\""));
    assert!(contents.contains("\"build\" = { program = \"npm\", args = [\"run\", \"build\"]"));
    assert!(contents.contains("\"run\" = { program = \"npm\", args = [\"run\", \"start\"]"));
    assert!(contents.contains("\"lint\" = { program = \"npm\", args = [\"run\", \"lint\"]"));
}

#[test]
fn init_imports_pyproject_poetry() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join("pyproject.toml"),
        r#"[tool.poetry]
name = "demo"
version = "0.1.0"
"#,
    )
    .expect("write pyproject");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--import"])
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("name = \"demo\""));
    assert!(contents.contains("program = \"poetry\""));
}

#[test]
fn init_imports_makefile_targets() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join("Makefile"),
        "build:\n\tcargo build\n\ntest:\n\tcargo test\n",
    )
    .expect("write makefile");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--import"])
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("program = \"make\""));
    assert!(contents.contains("\"build\" = { program = \"make\", args = [\"build\"]"));
    assert!(contents.contains("\"test\" = { program = \"make\", args = [\"test\"]"));
}

#[test]
fn init_detected_template_drives_interactive_prompts() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join("package.json"),
        "{\n  \"name\": \"demo\"\n}\n",
    )
    .expect("write marker");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--detect", "--interactive"])
        .write_stdin(
            "demo
.

n
n
n
",
        )
        .assert()
        .stdout(contains("Template [node]:"))
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("program = \"npm\""));
    assert!(contents.contains("run = { program = \"npm\""));
}

#[test]
fn init_supports_extended_template_catalog() {
    let cases = [
        ("bun", "program = \"bun\""),
        ("deno", "program = \"deno\""),
        ("nextjs", "program = \"npm\""),
        ("vite", "program = \"npm\""),
        ("turbo", "program = \"turbo\""),
        ("nx", "program = \"nx\""),
        ("pnpm", "program = \"pnpm\""),
        ("yarn", "program = \"yarn\""),
        ("django", "manage.py"),
        ("fastapi", "uvicorn"),
        ("flask", "flask"),
        ("poetry", "program = \"poetry\""),
        ("hatch", "program = \"hatch\""),
        ("pixi", "program = \"pixi\""),
        ("uv", "program = \"uv\""),
        ("cargo-workspace", "default workspace member"),
        ("java-gradle", "gradlew"),
        ("java-maven", "mvn"),
        ("kotlin-gradle", "gradlew"),
        ("dotnet", "dotnet"),
        ("php-composer", "composer"),
        ("ruby-bundler", "bundle"),
        ("rails", "bin"),
        ("laravel", "artisan"),
        ("terraform", "terraform"),
        ("helm", "helm"),
        ("docker-compose", "docker"),
        ("cmake-ninja", "-G"),
    ];

    for case in cases {
        let temp = TempDir::new().expect("temp dir");
        Command::cargo_bin("mbr")
            .expect("binary")
            .current_dir(temp.path())
            .args(["init", "--template", case.0])
            .assert()
            .success();

        let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
        assert!(contents.contains(case.1));
        if case.0 == "cmake-ninja" {
            assert!(contents.contains("Ninja"));
        }
    }
}

#[test]
fn templates_match_snapshot() {
    let temp = TempDir::new().expect("temp dir");
    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("templates")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let actual = String::from_utf8(output).expect("utf8 output");
    let expected = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/TEMPLATE_CATALOG.txt"));
    assert_eq!(actual, expected);
}

#[test]
fn templates_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "templates"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "templates");
    assert_eq!(value["count"], value["templates"].as_array().unwrap().len());
    assert!(value["count"].as_u64().unwrap() > 0);
}

#[test]
fn which_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    let nested = mkdir(temp.path(), "nested");
    write_config(
        temp.path(),
        r#"
[project]
root = "."

[profiles.dev]
[profiles.dev.commands]
build = "echo ok"

[commands]
build = "echo ok"
"#,
    );
    write_config(&nested, "[commands]\nbuild = \"echo ok\"\n");

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(&nested)
        .env("MBR_PROFILE", "dev")
        .args(["--json", "which"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "which");
    assert!(value["config"].is_string());
    assert!(value["root"].is_string());
    assert_eq!(value["selected_profile"], "dev");
    assert_eq!(value["config_chain"].as_array().unwrap().len(), 2);
}

#[test]
fn doctor_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        "[commands]\nbuild = \"echo ok\"\ntest = \"echo ok\"\nrun = \"echo ok\"\nfmt = \"echo ok\"\nclean = \"echo ok\"\nci = \"echo ok\"\n",
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "warn");
    assert_eq!(value["command"], "doctor");
    assert!(value["warnings"].as_array().is_some());
    assert!(value["suggestions"].as_array().is_some());
}

#[test]
fn doctor_suggests_fixes_for_missing_tools_and_env_files() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
env_file = ".env.missing"

[commands]
build = { program = "definitely-not-on-path-12345" }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains(
            "warning: command `build` program `definitely-not-on-path-12345` was not found on PATH",
        ))
        .stdout(contains("warning: env file `.env.missing` was not found"))
        .stdout(contains(
            "suggestion: install `definitely-not-on-path-12345` or add it to PATH",
        ))
        .stdout(contains(
            "suggestion: create `.env.missing` or update `env_file` in the config",
        ));
}

#[test]
fn doctor_reports_explicit_requirements() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = "echo ok"
test = "echo ok"
run = "echo ok"
fmt = "echo ok"
clean = "echo ok"
ci = "echo ok"
lint = "echo ok"

[requirements]
tools = ["definitely-not-on-path-12345"]
files = ["required.txt"]
env = ["REQUIRED_TOKEN"]
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains(
            "warning: required tool `definitely-not-on-path-12345` was not found on PATH",
        ))
        .stdout(contains(
            "warning: required file `required.txt` was not found",
        ))
        .stdout(contains(
            "warning: required env var `REQUIRED_TOKEN` was not set",
        ))
        .stdout(contains(
            "suggestion: install `definitely-not-on-path-12345` or update `[requirements].tools`",
        ))
        .stdout(contains(
            "suggestion: create `required.txt` or update `[requirements].files`",
        ))
        .stdout(contains(
            "suggestion: set `REQUIRED_TOKEN` or update `[requirements].env`",
        ));
}

#[test]
fn doctor_fix_creates_missing_env_files() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
env_file = ".env.ci"

[commands]
build = "echo ok"
test = "echo ok"
run = "echo ok"
fmt = "echo ok"
clean = "echo ok"
ci = "echo ok"
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["doctor", "--fix"])
        .assert()
        .success()
        .stdout(contains("fixed: created env file"));

    assert!(temp.path().join(".env.ci").exists());
}

#[test]
fn init_list_templates_prints_catalog_without_writing() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--list-templates"])
        .assert()
        .success()
        .stdout(contains("rust - Rust projects"));

    assert!(!temp.path().join(".mbr.toml").exists());
}

#[test]
fn all_templates_render_valid_configs() {
    let temp = TempDir::new().expect("temp dir");
    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("templates")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let catalog = String::from_utf8(output).expect("utf8 output");

    for line in catalog.lines() {
        let Some((name, _description)) = line.split_once(" - ") else {
            continue;
        };

        let template_dir = TempDir::new().expect("template dir");
        Command::cargo_bin("mbr")
            .expect("binary")
            .current_dir(template_dir.path())
            .args(["init", "--template", name])
            .assert()
            .success();

        Command::cargo_bin("mbr")
            .expect("binary")
            .current_dir(template_dir.path())
            .arg("validate")
            .assert()
            .success();
    }
}

#[test]
fn init_supports_interactive_prompts() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--interactive"])
        .write_stdin("demo\napp\nnode\n")
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("name = \"demo\""));
    assert!(contents.contains("root = \"app\""));
    assert!(contents.contains("program = \"npm\""));
}

#[test]
fn init_interactive_prompts_are_template_specific() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--interactive"])
        .write_stdin(
            "demo
.
rust
n
",
        )
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("docs = { program = \"cargo\", args = [\"doc\"]"));
    assert!(contents.contains("lint = { program = \"cargo\", args = [\"clippy\""));
}

#[test]
fn init_interactive_safe_mode_rejects_shell_templates() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--interactive"])
        .write_stdin("demo\n.\ngeneric\nn\nn\nn\nn\ny\n")
        .assert()
        .failure()
        .stderr(contains("safe init template forbids shell command"));

    assert!(!temp.path().join(".mbr.toml").exists());
}

#[test]
fn init_generic_template_can_include_optional_commands() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--interactive"])
        .write_stdin("demo\n.\ngeneric\ny\ny\ny\ny\nn\n")
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("docs = \"echo docs\""));
    assert!(contents.contains("dev = \"echo dev\""));
    assert!(contents.contains("lint = \"echo lint\""));
    assert!(contents.contains("typecheck = \"echo typecheck\""));
}

#[test]
fn init_uses_custom_template_file() {
    let temp = TempDir::new().expect("temp dir");
    let template = temp.path().join("custom-template.toml");
    fs::write(
        &template,
        r#"[project]
name = "{{project_name}}"
root = "{{project_root}}"

[commands]
build = "echo {{template}}"
"#,
    )
    .expect("write template");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--template-file", template.to_str().expect("path")])
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("name = \"example\""));
    assert!(contents.contains("root = \".\""));
    assert!(contents.contains("echo rust"));
}

#[test]
fn init_uses_custom_template_directory() {
    let temp = TempDir::new().expect("temp dir");
    let template_dir = temp.path().join("template-dir");
    fs::create_dir_all(&template_dir).expect("create template dir");
    fs::write(
        template_dir.join("template.toml"),
        r#"[project]
name = "{{project_name}}"
root = "{{project_root}}"

[commands]
build = "echo {{template}}"
"#,
    )
    .expect("write template");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "init",
            "--template-file",
            template_dir.to_str().expect("path"),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(temp.path().join(".mbr.toml")).expect("read config");
    assert!(contents.contains("name = \"example\""));
    assert!(contents.contains("root = \".\""));
    assert!(contents.contains("echo rust"));
}

#[test]
fn init_rejects_invalid_custom_template() {
    let temp = TempDir::new().expect("temp dir");
    let template = temp.path().join("invalid-template.toml");
    fs::write(&template, "[project\nname = 'broken'\n").expect("write invalid template");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["init", "--template-file", template.to_str().expect("path")])
        .assert()
        .failure()
        .stderr(contains("generated init template is invalid"));

    assert!(!temp.path().join(".mbr.toml").exists());
}

#[test]
fn list_outputs_command_names() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
clean = { program = "cargo", args = ["clean"] }
ci = { program = "cargo", args = ["test"] }
fmt = { program = "cargo", args = ["fmt"] }
lint = { program = "cargo", args = ["clippy"] }
test = { program = "cargo", args = ["test"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(contains("build"))
        .stdout(contains("clean"))
        .stdout(contains("ci"))
        .stdout(contains("fmt"))
        .stdout(contains("lint"))
        .stdout(contains("test"));
}

#[test]
fn list_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
clean = { program = "cargo", args = ["clean"] }
"#,
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "list");
    assert!(value["commands"].as_array().is_some());
}

#[test]
fn list_shows_command_descriptions() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"], description = "Compile the project" }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(contains("build - Compile the project"));
}

#[test]
fn which_reports_config_and_root() {
    let temp = TempDir::new().expect("temp dir");
    let nested = mkdir(temp.path(), "nested");
    write_config(
        temp.path(),
        r#"
[project]
root = "."

[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );
    write_config(&nested, "[commands]\nbuild = \"echo nested\"\n");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(&nested)
        .arg("which")
        .assert()
        .success()
        .stdout(contains("config:"))
        .stdout(contains("root:"))
        .stdout(contains("chain:"))
        .stdout(contains("profile: (none)"));
}

#[test]
fn dry_run_prints_rendered_command() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--dry-run", "build", "--", "--release"])
        .assert()
        .success()
        .stdout(contains("cargo build --release"));
}

#[test]
fn dry_run_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "--dry-run", "build"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "dry-run");
    assert_eq!(value["name"], "build");
    assert!(value["rendered"].is_string());
}

#[test]
fn doctor_reports_config_status() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("config:"))
        .stdout(contains("warning:"));
}

#[test]
fn doctor_strict_fails_when_warnings_exist() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["doctor", "--strict"])
        .assert()
        .failure()
        .stdout(contains("missing fmt command"));
}

#[test]
fn show_reports_source_provenance() {
    let temp = TempDir::new().expect("temp dir");
    let nested = mkdir(temp.path(), "nested");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build", "--release"], cwd = "nested", description = "Compile the project" }

[profiles.dev]
[profiles.dev.commands]
build = { program = "cargo", args = ["build", "--release"], description = "Profile build", windows = { args = ["build", "--locked"] }, unix = { args = ["build", "--locked"] } }
"#,
    );
    write_config(
        &nested,
        "[commands]\nbuild = { program = \"cargo\", args = [\"build\", \"--release\"], cwd = \"nested\", description = \"Child build\" }\n",
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(nested.clone())
        .env("MBR_PROFILE", "dev")
        .args(["show", "--source", "build"])
        .assert()
        .success()
        .stdout(contains("name: build"))
        .stdout(contains("cargo build --locked"))
        .stdout(contains("cwd:"))
        .stdout(contains("Profile build"))
        .stdout(contains("config chain:"))
        .stdout(contains("source: base config:"))
        .stdout(contains("source: child config:"))
        .stdout(contains("source: profile: dev"))
        .stdout(contains("source: platform override:"));

    assert!(nested.exists());
}

#[test]
fn show_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "show", "build"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "show");
    assert_eq!(value["name"], "build");
    assert!(value["sources"].as_array().is_some());
}

#[test]
fn command_cwd_is_respected() {
    let temp = TempDir::new().expect("temp dir");
    let nested = mkdir(temp.path(), "nested");
    let expected = temp
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temp dir name")
        .to_string();
    write_config(
        temp.path(),
        &format!(
            "[commands]\nrun = {{ {}, cwd = \"nested\" }}\n",
            cwd_command_spec()
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("run")
        .assert()
        .success()
        .stdout(contains(expected));

    assert!(nested.exists());
}

#[test]
fn command_timeout_fails_cleanly() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {{ {}, timeout = 1 }}\n",
            sleep_command_spec()
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .failure()
        .stderr(contains("timed out"));
}

#[test]
fn workspace_override_is_respected() {
    let temp = TempDir::new().expect("temp dir");
    let workspace = mkdir(temp.path(), "workspace");
    write_config(
        &workspace,
        &format!(
            "[commands]\nbuild = {}\n",
            print_command_spec("workspace-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--workspace", "workspace", "build"])
        .assert()
        .success()
        .stdout(contains("workspace-ok"));
}

#[test]
fn parallel_runs_commands_concurrently() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\none = {}\ntwo = {}\nthree = {}\n",
            sleep_and_print_command_spec("one"),
            sleep_and_print_command_spec("two"),
            sleep_and_print_command_spec("three")
        ),
    );

    let start = std::time::Instant::now();
    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["parallel", "one", "two", "three"])
        .assert()
        .success()
        .stdout(contains("[one] one"))
        .stdout(contains("[two] two"))
        .stdout(contains("[three] three"))
        .stderr(contains(
            "[mbr] summary: command=parallel status=ok count=3",
        ));

    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn parallel_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\none = {}\ntwo = {}\n",
            sleep_and_print_command_spec("one"),
            sleep_and_print_command_spec("two")
        ),
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "parallel", "one", "two"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "parallel");
    assert!(value["parallel"].as_array().is_some());
}

#[test]
fn parallel_reports_failed_command_summary() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\none = {}\ntwo = {}\n",
            failing_command_spec("one-fail", 4),
            print_command_spec("two-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["parallel", "one", "two"])
        .assert()
        .failure()
        .stderr(contains("[mbr] failed: command=one | exit=4"))
        .stderr(contains("duration="));
}

#[test]
fn child_config_inherits_parent_commands() {
    let temp = TempDir::new().expect("temp dir");
    let nested = mkdir(temp.path(), "nested");
    let expected_root = temp
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temp dir name")
        .to_string();
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {{ {}, description = \"Inherit build from parent\" }}\ntest = {}\n",
            cwd_command_spec(),
            print_command_spec("parent-test")
        ),
    );
    write_config(
        &nested,
        &format!("[commands]\nrun = {}\n", print_command_spec("child-run")),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(nested.clone())
        .arg("build")
        .assert()
        .success()
        .stdout(contains(expected_root));

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(nested)
        .arg("test")
        .assert()
        .success()
        .stdout(contains("parent-test"));
}

#[test]
fn command_extends_inherits_base_flags() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
release = { extends = "build", args = ["--release"], description = "Release build" }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["show", "release"])
        .assert()
        .success()
        .stdout(contains("cargo build --release"))
        .stdout(contains("Release build"));
}

#[test]
fn command_extends_can_replace_args() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build", "--locked"] }
release = { extends = "build", args_mode = "replace", args = ["build", "--release"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["show", "release"])
        .assert()
        .success()
        .stdout(contains("cargo build --release"));
}

#[test]
fn command_extends_can_replace_env() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {{ {}, env = {{ BASE = \"base\", KEEP = \"keep\" }} }}\nrelease = {{ extends = \"build\", env_mode = \"replace\", env = {{ CHILD = \"child\" }} }}\n",
            env_values_command_spec()
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["exec", "release"])
        .assert()
        .success()
        .stdout(contains("||child"));
}

#[test]
fn os_specific_overrides_are_applied() {
    let temp = TempDir::new().expect("temp dir");
    let expected = if cfg!(windows) {
        "windows-ok"
    } else {
        "unix-ok"
    };
    let override_spec = if cfg!(windows) {
        r#"windows = { program = "cmd", args = ["/C", "echo windows-ok"] }"#
    } else {
        r#"unix = { program = "sh", args = ["-c", "printf unix-ok"] }"#
    };
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {{ program = \"cargo\", args = [\"build\"], {} }}\n",
            override_spec
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains(expected));
}

#[test]
fn profile_overrides_commands_and_env() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {}\n\n[profiles.dev]\nenv = {{ PROFILE = \"dev\" }}\n[profiles.dev.commands]\nbuild = {}\n",
            print_command_spec("base-ok"),
            print_command_spec("profile-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .env("MBR_PROFILE", "dev")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains("profile-ok"));
}

#[test]
fn profile_flag_overrides_environment() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {}\n\n[profiles.dev]\n[profiles.dev.commands]\nbuild = {}\n\n[profiles.ci]\n[profiles.ci.commands]\nbuild = {}\n",
            print_command_spec("base-ok"),
            print_command_spec("dev-ok"),
            print_command_spec("ci-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .env("MBR_PROFILE", "dev")
        .current_dir(temp.path())
        .args(["--profile", "ci", "build"])
        .assert()
        .success()
        .stdout(contains("ci-ok"));
}

#[test]
fn project_env_file_is_loaded() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join(".env.ci"), "FROM_FILE=file-value\n").expect("write env file");
    write_config(
        temp.path(),
        &format!(
            "env_file = \".env.ci\"\n[commands]\nbuild = {}\n",
            single_env_command_spec("FROM_FILE")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains("file-value"));
}

#[test]
fn profile_env_file_is_loaded() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join(".env.ci"), "FROM_PROFILE=file-value\n").expect("write env file");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {}\n\n[profiles.ci]\nenv_file = \".env.ci\"\n[profiles.ci.commands]\nbuild = {}\n",
            single_env_command_spec("FROM_PROFILE"),
            single_env_command_spec("FROM_PROFILE")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .env("MBR_PROFILE", "ci")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains("file-value"));
}

#[test]
fn list_verbose_prints_command_details() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"], description = "Compile" }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["list", "--verbose"])
        .assert()
        .success()
        .stdout(contains("command: cargo build"))
        .stdout(contains("description: Compile"));
}

#[test]
fn explain_prints_command_type() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
fmt = { program = "cargo", args = ["fmt"] }
lint = { program = "cargo", args = ["clippy"] }
ci = { steps = ["fmt", "lint"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["explain", "ci"])
        .assert()
        .success()
        .stdout(contains("type: pipeline"))
        .stdout(contains("steps: fmt -> lint"))
        .stdout(contains("source: base config:"));
}

#[test]
fn explain_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
fmt = { program = "cargo", args = ["fmt"] }
lint = { program = "cargo", args = ["clippy"] }
ci = { steps = ["fmt", "lint"] }
"#,
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "explain", "ci"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "explain");
    assert_eq!(value["name"], "ci");
    assert!(value["sources"].as_array().is_some());
}

#[test]
fn safe_mode_rejects_shell_commands() {
    let temp = TempDir::new().expect("temp dir");
    write_config(temp.path(), "[commands]\nbuild = \"echo unsafe\"\n");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--safe", "build"])
        .assert()
        .failure()
        .stderr(contains("safe mode forbids shell command `build`"));
}

#[test]
fn dotenv_file_is_loaded_before_execution() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join(".env"), "FROM_FILE=file-value\n").expect("write env file");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nbuild = {}\n",
            single_env_command_spec("FROM_FILE")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains("file-value"));
}

#[test]
fn command_retries_failed_attempts() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nbuild = {}\n", retrying_command_spec()),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stdout(contains("retry-ok"));
}

#[test]
fn workspace_lists_discovered_projects() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--list"])
        .assert()
        .success()
        .stdout(contains("name: first"))
        .stdout(contains("name: second"));
}

#[test]
fn workspace_filters_projects_by_name() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--name", "first", "--list"])
        .assert()
        .success()
        .stdout(contains("name: first"))
        .stdout(predicates::str::contains("name: second").not());
}

#[test]
fn workspace_filters_projects_by_changed_files() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    run_git(temp.path(), &["init", "-q"]);
    run_git(temp.path(), &["config", "user.name", "mbr"]);
    run_git(temp.path(), &["config", "user.email", "mbr@example.com"]);
    run_git(temp.path(), &["add", "."]);
    run_git(temp.path(), &["commit", "-q", "-m", "initial"]);

    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-changed")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--changed-only", "--list"])
        .assert()
        .success()
        .stdout(contains("name: second"))
        .stdout(predicates::str::contains("name: first").not());

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--changed-only", "build"])
        .assert()
        .success()
        .stdout(contains("[second] second-changed"))
        .stdout(predicates::str::contains("first-ok").not())
        .stderr(contains(
            "[mbr] summary: command=workspace build status=ok count=1",
        ));

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--changed-only", "--since", "HEAD", "build"])
        .assert()
        .success()
        .stdout(contains("[second] second-changed"))
        .stdout(predicates::str::contains("first-ok").not())
        .stderr(contains(
            "[mbr] summary: command=workspace build status=ok count=1",
        ));
}

#[test]
fn workspace_runs_command_in_named_projects_only() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--name", "first", "build"])
        .assert()
        .success()
        .stdout(contains("[first] first-ok"))
        .stdout(predicates::str::contains("second-ok").not())
        .stderr(contains(
            "[mbr] summary: command=workspace build status=ok count=1",
        ));
}

#[test]
fn workspace_runs_command_in_each_project() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "build"])
        .assert()
        .success()
        .stdout(contains("[first] first-ok"))
        .stdout(contains("[second] second-ok"))
        .stderr(contains(
            "[mbr] summary: command=workspace build status=ok count=2",
        ));
}

#[test]
fn workspace_jobs_runs_projects_concurrently() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            sleep_and_print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            sleep_and_print_command_spec("second-ok")
        ),
    );

    let started = std::time::Instant::now();
    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--jobs", "2", "build"])
        .assert()
        .success();

    assert!(started.elapsed() < std::time::Duration::from_secs(4));
}

#[test]
fn workspace_fail_fast_stops_after_first_failure() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            failing_command_spec("first-fail", 7)
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--fail-fast", "build"])
        .assert()
        .failure()
        .stdout(contains("first-fail"))
        .stdout(predicates::str::contains("second-ok").not())
        .stderr(contains(
            "[mbr] summary: command=workspace build status=warn count=1",
        ));
}

#[test]
fn workspace_order_name_changes_execution_order() {
    let temp = TempDir::new().expect("temp dir");
    let zeta_dir = mkdir(temp.path(), "zzz");
    let alpha_dir = mkdir(temp.path(), "aaa");
    write_config(
        &zeta_dir,
        &format!(
            "[project]\nname = \"zeta\"\n[commands]\nbuild = {}\n",
            print_command_spec("zeta-ok")
        ),
    );
    write_config(
        &alpha_dir,
        &format!(
            "[project]\nname = \"alpha\"\n[commands]\nbuild = {}\n",
            print_command_spec("alpha-ok")
        ),
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "--order", "name", "build"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).expect("utf8 output");
    let alpha_pos = stdout.find("alpha-ok").expect("alpha output");
    let zeta_pos = stdout.find("zeta-ok").expect("zeta output");
    assert!(alpha_pos < zeta_pos);
}

#[test]
fn workspace_json_has_stable_envelope() {
    let temp = TempDir::new().expect("temp dir");
    let first = mkdir(temp.path(), "first");
    let second = mkdir(temp.path(), "second");
    write_config(
        &first,
        &format!(
            "[project]\nname = \"first\"\n[commands]\nbuild = {}\n",
            print_command_spec("first-ok")
        ),
    );
    write_config(
        &second,
        &format!(
            "[project]\nname = \"second\"\n[commands]\nbuild = {}\n",
            print_command_spec("second-ok")
        ),
    );

    let output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["--json", "workspace", "build"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "workspace");
    assert!(value["projects"].is_null() || value["projects"].as_array().is_some());
}

#[test]
fn workspace_reports_failed_command_summary() {
    let temp = TempDir::new().expect("temp dir");
    let workspace = mkdir(temp.path(), "project");
    write_config(
        &workspace,
        &format!(
            "[project]\nname = \"project\"\n[commands]\nbuild = {}\n",
            failing_command_spec("workspace-fail", 3)
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["workspace", "build"])
        .assert()
        .failure()
        .stderr(contains(
            "[mbr] failed: project=project | command=build | exit=3",
        ))
        .stderr(contains("duration="));
}

#[test]
fn package_creates_an_archive() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("README.txt"), "hello").expect("write file");
    write_config(
        temp.path(),
        &format!(
            "[project]\nname = \"demo\"\n[commands]\nbuild = {}\n",
            print_command_spec("build-ok")
        ),
    );
    let output = temp.path().join(if cfg!(windows) {
        "demo.zip"
    } else {
        "demo.tar.gz"
    });

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["package", "--output", output.to_string_lossy().as_ref()])
        .assert()
        .success();

    let json_output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--json",
            "package",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&json_output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "package");
    assert!(value["output"].is_string());

    assert!(output.exists());
    assert!(archive_contains_file(&output, "README.txt"));
    assert!(archive_contains_file(&output, ".mbr.toml"));
}

#[test]
fn release_runs_build_test_and_packages() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("README.txt"), "hello").expect("write file");
    write_config(
        temp.path(),
        &format!(
            "[project]\nname = \"demo\"\n[commands]\nbuild = {}\ntest = {}\n",
            print_command_spec("build-ok"),
            print_command_spec("test-ok")
        ),
    );
    let output = temp.path().join(if cfg!(windows) {
        "demo.zip"
    } else {
        "demo.tar.gz"
    });

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["release", "--output", output.to_string_lossy().as_ref()])
        .assert()
        .success()
        .stdout(contains("build-ok"))
        .stdout(contains("test-ok"))
        .stderr(contains("[mbr] summary: command=release status=ok count=2"));

    let json_output = Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args([
            "--json",
            "release",
            "--output",
            output.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&json_output).expect("json output");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["command"], "release");
    assert!(value["output"].is_string());

    assert!(output.exists());
}

#[test]
fn completions_prints_shell_script() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("mbr"));
}

#[test]
fn manpage_prints_manual_page() {
    let temp = TempDir::new().expect("temp dir");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("manpage")
        .assert()
        .success()
        .stdout(contains("mbr"))
        .stdout(contains("SYNOPSIS"));
}

#[test]
fn warns_when_project_name_is_missing() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!("[commands]\nbuild = {}\n", print_command_spec("build-ok")),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .success()
        .stderr(contains("project name is not set"));
}

#[test]
fn pipeline_command_runs_named_steps() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        &format!(
            "[commands]\nfmt = {}\nlint = {}\ntest = {}\nci = {{ steps = [\"fmt\", \"lint\", \"test\"] }}\n",
            print_command_spec("fmt-ok"),
            print_command_spec("lint-ok"),
            print_command_spec("test-ok")
        ),
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("ci")
        .assert()
        .success()
        .stdout(contains("fmt-ok"))
        .stdout(contains("lint-ok"))
        .stdout(contains("test-ok"));
}

#[test]
fn show_prints_pipeline_summary() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
fmt = { program = "cargo", args = ["fmt"] }
lint = { program = "cargo", args = ["clippy"] }
ci = { steps = ["fmt", "lint"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["show", "ci"])
        .assert()
        .success()
        .stdout(contains("fmt -> lint"));
}

#[test]
fn doctor_flags_missing_common_commands() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "cargo", args = ["build"] }
test = { program = "cargo", args = ["test"] }
run = { program = "cargo", args = ["run"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(contains("missing fmt command"))
        .stdout(contains("missing clean command"))
        .stdout(contains("missing ci command"));
}
