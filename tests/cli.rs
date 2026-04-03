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

#[test]
fn build_runs_configured_command() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = { program = "sh", args = ["-c", "printf build-ok"] }
test = { program = "sh", args = ["-c", "printf test-ok"] }
run = { program = "sh", args = ["-c", "printf run-ok"] }
"#,
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
        r#"
[commands]
build = { program = "sh", args = ["-c", "printf '%s|%s' \"$1\" \"$2\"", "sh"] }
"#,
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
        r#"
[commands]
lint = { program = "sh", args = ["-c", "printf lint-ok"] }
build = { program = "sh", args = ["-c", "printf build-ok"] }
"#,
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
        r#"
[commands]
fmt = { program = "sh", args = ["-c", "printf fmt-ok"] }
clean = { program = "sh", args = ["-c", "printf clean-ok"] }
ci = { program = "sh", args = ["-c", "printf ci-ok"] }
"#,
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
        r#"
[commands]
run = { program = "sh", args = ["-c", "printf run-ok"] }
"#,
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
        r#"
[commands]
build = { program = "sh", args = ["-c", "printf build-ok"] }
"#,
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
