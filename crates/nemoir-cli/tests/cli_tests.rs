use std::process::Command;

fn nemoir_binary() -> Command {
    let path = std::env::var("CARGO_BIN_EXE_nemoir-dsl").expect("CARGO_BIN_EXE_nemoir-dsl not set");
    Command::new(path)
}

#[test]
fn cli_check_valid() {
    let output = nemoir_binary()
        .arg("check")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nemoir-dsl-fe/tests/fixtures/coding-agent.nemo"
        ))
        .output()
        .expect("should run nemoir check");
    assert!(output.status.success(), "check should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OK"), "should print OK");
}

#[test]
fn cli_check_invalid() {
    let output = nemoir_binary()
        .arg("check")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nemoir-dsl-fe/tests/fixtures/invalid/no_entry.nemo"
        ))
        .output()
        .expect("should run nemoir check");
    assert!(
        !output.status.success(),
        "check should fail for invalid file"
    );
}

#[test]
fn cli_lower_stdout() {
    let output = nemoir_binary()
        .arg("lower")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nemoir-dsl-fe/tests/fixtures/coding-agent.nemo"
        ))
        .output()
        .expect("should run nemoir lower");
    assert!(output.status.success(), "lower should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ir_version"),
        "should contain ir_version in YAML"
    );
    assert!(stdout.contains("CodingAgent"), "should contain workflow id");
}

#[test]
fn cli_lower_to_file() {
    let out_path = std::env::temp_dir().join("nemoir_cli_test_out.yml");
    let output = nemoir_binary()
        .arg("lower")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nemoir-dsl-fe/tests/fixtures/coding-agent.nemo"
        ))
        .arg("-o")
        .arg(&out_path)
        .output()
        .expect("should run nemoir lower -o");
    assert!(output.status.success(), "lower -o should succeed");
    let contents = std::fs::read_to_string(&out_path).expect("should read output file");
    assert!(
        contents.contains("ir_version"),
        "should contain ir_version in file"
    );
    assert!(
        contents.contains("CodingAgent"),
        "should contain workflow id"
    );
    std::fs::remove_file(&out_path).ok();
}
