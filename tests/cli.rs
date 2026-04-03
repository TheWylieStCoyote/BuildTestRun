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
build = "printf build-ok"
test = "printf test-ok"
run = "printf run-ok"
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
fn discovers_config_from_subdirectory() {
    let temp = TempDir::new().expect("temp dir");
    write_config(
        temp.path(),
        r#"
[commands]
build = "printf build-ok"
test = "printf test-ok"
run = "printf run-ok"
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
fn reports_invalid_toml() {
    let temp = TempDir::new().expect("temp dir");
    write_config(temp.path(), "[commands\nbuild = \"oops\"");

    Command::cargo_bin("mbr")
        .expect("binary")
        .current_dir(temp.path())
        .arg("build")
        .assert()
        .failure()
        .stderr(contains("failed to parse config"));
}
