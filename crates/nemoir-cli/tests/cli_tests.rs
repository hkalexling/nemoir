use std::process::Command;

fn nemoir_binary() -> Command {
    let path = std::env::var("CARGO_BIN_EXE_nemo").expect("CARGO_BIN_EXE_nemo not set");
    Command::new(path)
}

fn coding_agent_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/coding-agent.nemo"
    )
    .to_string()
}

fn dup_stage_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/invalid/duplicate_stage.nemo"
    )
    .to_string()
}

#[test]
fn cli_check_valid() {
    let output = nemoir_binary()
        .arg("check")
        .arg(coding_agent_path())
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
        .arg(dup_stage_path())
        .output()
        .expect("should run nemoir check");
    assert!(
        !output.status.success(),
        "check should fail for invalid file"
    );
}

#[test]
fn cli_compile_default_none_no_artifact() {
    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .output()
        .expect("should run nemoir compile (default none)");

    assert!(
        output.status.success(),
        "compile with default none should succeed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("IR validated successfully"),
        "should report successful validation"
    );
}

#[test]
fn cli_compile_default_none_dump_ir() {
    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--dump-ir")
        .output()
        .expect("should run nemoir compile --dump-ir (default none)");

    assert!(output.status.success(), "compile --dump-ir should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ir_version"),
        "stdout should contain YAML IR"
    );
    assert!(
        stdout.contains("CodingAgent"),
        "stdout should contain workflow id"
    );
    // With default none, should NOT have written any file
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("wrote:"),
        "default none should not write a file"
    );
}

#[test]
fn cli_compile_visualizer_creates_html() {
    let out_path = std::env::temp_dir().join("coding-agent.html");
    let _ = std::fs::remove_file(&out_path);

    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("visualizer")
        .arg("-o")
        .arg(&out_path)
        .output()
        .expect("should run nemoir compile --target visualizer");

    assert!(
        output.status.success(),
        "compile --target visualizer should succeed"
    );
    let html = std::fs::read_to_string(&out_path).expect("should read output file");
    assert!(html.contains("<!DOCTYPE html>"), "should be valid HTML");
    assert!(html.contains("CodingAgent"), "should contain workflow id");
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn cli_compile_visualizer_output_path() {
    let out_path = std::env::temp_dir().join("graph.html");
    let _ = std::fs::remove_file(&out_path);

    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("visualizer")
        .arg("-o")
        .arg(&out_path)
        .output()
        .expect("should run nemoir compile --target visualizer -o");

    assert!(output.status.success(), "compile -o should succeed");
    assert!(out_path.exists(), "output file should exist");
    let html = std::fs::read_to_string(&out_path).expect("should read output file");
    assert!(html.contains("CodingAgent"), "should contain workflow id");
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn cli_compile_visualizer_dump_ir() {
    let out_path = std::env::temp_dir().join("nemoir_dump_ir_test.html");
    let _ = std::fs::remove_file(&out_path);

    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("visualizer")
        .arg("--dump-ir")
        .arg("-o")
        .arg(&out_path)
        .output()
        .expect("should run nemoir compile --target visualizer --dump-ir");

    assert!(output.status.success(), "compile --dump-ir should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ir_version"),
        "stdout should contain YAML IR"
    );
    assert!(
        stdout.contains("CodingAgent"),
        "stdout should contain workflow id"
    );

    let html = std::fs::read_to_string(&out_path).expect("should read output file");
    assert!(
        html.contains("CodingAgent"),
        "HTML should contain workflow id"
    );
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn cli_compile_unknown_target() {
    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("bogus")
        .output()
        .expect("should run nemoir compile --target bogus");

    assert!(!output.status.success(), "unknown target should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown compile target"),
        "should mention unknown target"
    );
    assert!(
        stderr.contains("none"),
        "should list 'none' as supported target"
    );
    assert!(
        stderr.contains("visualizer"),
        "should list 'visualizer' as supported target"
    );
}

#[test]
fn cli_compile_visualizer_stdin_no_output_fails() {
    let source = std::fs::read_to_string(coding_agent_path()).expect("should read source");
    let mut child = nemoir_binary()
        .arg("compile")
        .arg("-")
        .arg("--target")
        .arg("visualizer")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("should spawn nemoir");

    {
        let stdin = child.stdin.as_mut().expect("should get stdin");
        use std::io::Write;
        stdin
            .write_all(source.as_bytes())
            .expect("should write to stdin");
    }

    let output = child.wait_with_output().expect("should wait for output");

    assert!(
        !output.status.success(),
        "stdin with visualizer and no --output should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires --output"),
        "should mention --output is required: {}",
        stderr
    );
}
