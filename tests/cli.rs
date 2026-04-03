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
        r#"{ program = "cmd", args = ["/C", "echo %1^|%2"] }"#.to_string()
    } else {
        r#"{ program = "sh", args = ["-c", "printf '%s|%s' \"$1\" \"$2\"", "sh"] }"#.to_string()
    }
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
