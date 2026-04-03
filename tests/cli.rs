use assert_cmd::Command;
use predicates::str::contains;
use std::{fs, path::Path};
use tempfile::TempDir;

fn write_config(dir: &Path, body: &str) {
    fs::write(dir.join(".mbr.toml"), body).expect("write config");
}

fn mkdir(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(&path).expect("create dir");
    path
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
        .stdout(contains("build-ok"));
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
        ("cargo-workspace", "--workspace"),
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
    write_config(
        temp.path(),
        r#"
[project]
root = "."

[commands]
build = { program = "cargo", args = ["build"] }
"#,
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("which")
        .assert()
        .success()
        .stdout(contains("config:"))
        .stdout(contains("root:"));
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
fn show_prints_resolved_command_details() {
    let temp = TempDir::new().expect("temp dir");
    let nested = mkdir(temp.path(), "nested");
    write_config(
        temp.path(),
        "[commands]\nbuild = { program = \"cargo\", args = [\"build\", \"--release\"], cwd = \"nested\", description = \"Compile the project\" }\n",
    );

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .args(["show", "build"])
        .assert()
        .success()
        .stdout(contains("name: build"))
        .stdout(contains("cargo build --release"))
        .stdout(contains("cwd:"))
        .stdout(contains("Compile the project"));

    assert!(nested.exists());
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
        .success();

    assert!(start.elapsed() < std::time::Duration::from_secs(5));
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
        .stdout(contains("steps: fmt -> lint"));
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
        .stdout(contains("first-ok"))
        .stdout(contains("second-ok"));
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

    assert!(output.exists());
    assert!(archive_contains_file(&output, "README.txt"));
    assert!(archive_contains_file(&output, ".mbr.toml"));
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
