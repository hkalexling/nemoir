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

fn hint_tutor_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/hint_tutor.nemo"
    )
    .to_string()
}

fn http_fetch_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/http_fetch.nemo"
    )
    .to_string()
}

fn js_run_model_stage_err_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/js_run_model_stage_err.nemo"
    )
    .to_string()
}

fn js_run_positive_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/js_run_positive.nemo"
    )
    .to_string()
}

fn js_sandbox_positive_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/js_sandbox_positive.nemo"
    )
    .to_string()
}

fn js_sandbox_user_code_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/js_sandbox_user_code.nemo"
    )
    .to_string()
}

fn js_sandbox_code_json_type_err_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/js_sandbox_code_json_type_err.nemo"
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
    assert!(
        stderr.contains("python"),
        "should list 'python' as supported target"
    );
    assert!(
        stderr.contains("web"),
        "should list 'web' as supported target"
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

// ---------------------------------------------------------------------------
// `nemo compile --target python` integration tests
// ---------------------------------------------------------------------------

#[test]
fn cli_compile_python_creates_package() {
    let out_dir = std::env::temp_dir().join("nemoir_python_creates");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("python")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target python");

    assert!(
        output.status.success(),
        "compile --target python should succeed"
    );

    // Expected generated files.
    let init_path = out_dir.join("coding_agent").join("__init__.py");
    let manifest_path = out_dir.join("coding_agent").join("_manifest.py");
    let pyproject_path = out_dir.join("pyproject.toml");
    assert!(init_path.exists(), "should generate __init__.py");
    assert!(manifest_path.exists(), "should generate _manifest.py");
    assert!(pyproject_path.exists(), "should generate pyproject.toml");

    let init_src = std::fs::read_to_string(&init_path).expect("should read __init__.py");
    assert!(init_src.contains("Agent"), "init should expose Agent");
    assert!(
        init_src.contains("AgentInput"),
        "init should expose AgentInput"
    );

    let manifest_src = std::fs::read_to_string(&manifest_path).expect("should read _manifest.py");
    assert!(
        manifest_src.contains("workflow_id=\"CodingAgent\""),
        "manifest should encode WorkflowId"
    );

    let pyproject_src =
        std::fs::read_to_string(&pyproject_path).expect("should read pyproject.toml");
    assert!(pyproject_src.contains("name = \"coding-agent\""));
    assert!(pyproject_src.contains("packages = [\"coding_agent\"]"));

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_python_default_output_into_file_dir() {
    // Place a copy of coding-agent.nemo inside a temp dir and compile it without
    // `-o`, then assert the generated coding_agent/ lands next to the source.
    let work_dir = std::env::temp_dir().join("nemoir_python_default");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).expect("should create tempdir");
    let source = std::fs::read_to_string(coding_agent_path()).expect("should read source");
    let nemo_in_tmp = work_dir.join("coding-agent.nemo");
    std::fs::write(&nemo_in_tmp, source).expect("should write source copy");

    let output = nemoir_binary()
        .arg("compile")
        .arg(&nemo_in_tmp)
        .arg("--target")
        .arg("python")
        .output()
        .expect("should run nemo compile --target python (default output)");

    assert!(
        output.status.success(),
        "default-output compile should succeed"
    );

    let generated_pkg = work_dir.join("coding_agent");
    assert!(
        generated_pkg.join("__init__.py").exists(),
        "package dir should exist next to source file"
    );
    assert!(work_dir.join("pyproject.toml").exists());

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn cli_compile_python_stdin_no_output_fails() {
    let source = std::fs::read_to_string(coding_agent_path()).expect("should read source");
    let mut child = nemoir_binary()
        .arg("compile")
        .arg("-")
        .arg("--target")
        .arg("python")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("should spawn nemo");

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
        "stdin with python target and no --output should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires --output"),
        "should mention --output is required: {}",
        stderr
    );
}

#[test]
fn cli_compile_python_dump_ir_writes_package_and_dumps_yaml() {
    let out_dir = std::env::temp_dir().join("nemoir_python_dump_ir");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("python")
        .arg("--dump-ir")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target python --dump-ir");

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

    // Package should still be written next to the dumped IR.
    assert!(out_dir.join("coding_agent").join("__init__.py").exists());
    assert!(out_dir.join("pyproject.toml").exists());

    let _ = std::fs::remove_dir_all(&out_dir);
}

// ---------------------------------------------------------------------------
// `nemo compile --target web` integration tests
// ---------------------------------------------------------------------------

fn judge_candidate_path() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nemoir-dsl-fe/tests/fixtures/judge_candidate.nemo"
    )
    .to_string()
}

#[test]
fn cli_compile_web_positive_creates_package() {
    let out_dir = std::env::temp_dir().join("nemoir_web_creates");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(judge_candidate_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target web");

    assert!(
        output.status.success(),
        "compile --target web should succeed for judge_candidate"
    );

    // Expected generated files under the kebab-case package dir.
    let pkg = out_dir.join("judge-candidate");
    assert!(
        pkg.join("package.json").exists(),
        "package.json should exist"
    );
    assert!(
        pkg.join("vite.config.ts").exists(),
        "vite.config.ts should exist"
    );
    assert!(
        pkg.join("tsconfig.json").exists(),
        "tsconfig.json should exist"
    );
    assert!(pkg.join("index.html").exists(), "index.html should exist");
    assert!(
        pkg.join("netlify.toml").exists(),
        "netlify.toml should exist"
    );
    assert!(
        pkg.join("public").join("_headers").exists(),
        "public/_headers should exist"
    );
    assert!(
        pkg.join("src").join("workflow.json").exists(),
        "src/workflow.json should exist"
    );
    assert!(
        pkg.join("src").join("agent.ts").exists(),
        "src/agent.ts should exist"
    );
    assert!(
        pkg.join("src").join("main.tsx").exists(),
        "src/main.tsx should exist"
    );
    assert!(
        pkg.join("src").join("webllm.worker.ts").exists(),
        "src/webllm.worker.ts should exist"
    );

    // workflow.json must be valid JSON containing the workflow id.
    let wf_json = std::fs::read_to_string(pkg.join("src").join("workflow.json"))
        .expect("should read workflow.json");
    assert!(
        wf_json.contains("JudgeCandidate"),
        "workflow.json should contain workflow id"
    );

    // agent.ts must reference the workflow id and export Agent.
    let agent =
        std::fs::read_to_string(pkg.join("src").join("agent.ts")).expect("should read agent.ts");
    assert!(
        agent.contains("JudgeCandidate"),
        "agent.ts should contain workflow id"
    );
    assert!(
        agent.contains("export class Agent"),
        "agent.ts should export Agent class"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_negative_coding_agent_fails() {
    let out_dir = std::env::temp_dir().join("nemoir_web_negative");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(coding_agent_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target web (coding-agent)");

    assert!(
        !output.status.success(),
        "coding-agent must fail on web target (uses fs.* and path)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("web") || stderr.contains("not compatible"),
        "should mention web incompatibility: {stderr}"
    );

    // No output directory should have been created.
    assert!(
        !out_dir.join("coding-agent").exists(),
        "web backend must not write files on validation failure"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_negative_file_processor_fails() {
    // file-processor.nemo is in demos/, not in tests/fixtures. We use
    // coding-agent as the canonical negative case; this test verifies the
    // same failure mode holds for a fixture with path inputs.
    let out_dir = std::env::temp_dir().join("nemoir_web_fp_negative");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(judge_candidate_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target web");

    // judge_candidate should succeed (positive case) — this is a sanity
    // check that positive cases still work when a negative case was just run.
    assert!(
        output.status.success(),
        "judge_candidate should still succeed"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_stdin_no_output_fails() {
    let source = std::fs::read_to_string(judge_candidate_path()).expect("should read source");
    let mut child = nemoir_binary()
        .arg("compile")
        .arg("-")
        .arg("--target")
        .arg("web")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("should spawn nemo");

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
        "stdin with web target and no --output should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires --output"),
        "should mention --output is required: {stderr}"
    );
}

#[test]
fn cli_compile_web_default_output_into_file_dir() {
    let work_dir = std::env::temp_dir().join("nemoir_web_default");
    let _ = std::fs::remove_dir_all(&work_dir);
    std::fs::create_dir_all(&work_dir).expect("should create tempdir");
    let source = std::fs::read_to_string(judge_candidate_path()).expect("should read source");
    let nemo_in_tmp = work_dir.join("judge-candidate.nemo");
    std::fs::write(&nemo_in_tmp, source).expect("should write source copy");

    let output = nemoir_binary()
        .arg("compile")
        .arg(&nemo_in_tmp)
        .arg("--target")
        .arg("web")
        .output()
        .expect("should run nemo compile --target web (default output)");

    assert!(
        output.status.success(),
        "default-output web compile should succeed"
    );

    let generated_pkg = work_dir.join("judge-candidate");
    assert!(
        generated_pkg.join("package.json").exists(),
        "package dir should exist next to source file"
    );
    assert!(generated_pkg.join("netlify.toml").exists());

    let _ = std::fs::remove_dir_all(&work_dir);
}

#[test]
fn cli_compile_web_runtime_dependency_override() {
    let out_dir = std::env::temp_dir().join("nemoir_web_rtdep");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(judge_candidate_path())
        .arg("--target")
        .arg("web")
        .arg("--web-runtime-dependency")
        .arg("file:../../web/nemoir-runtime")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target web --web-runtime-dependency");

    assert!(output.status.success(), "compile should succeed");
    let pkg = out_dir.join("judge-candidate").join("package.json");
    let pkg_src = std::fs::read_to_string(&pkg).expect("should read package.json");
    assert!(
        pkg_src.contains(r#""@nemoir/web-runtime": "file:../../web/nemoir-runtime""#),
        "package.json should carry the overridden runtime dependency: {pkg_src}"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_ui_dependency_override() {
    let out_dir = std::env::temp_dir().join("nemoir_web_uidep");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(judge_candidate_path())
        .arg("--target")
        .arg("web")
        .arg("--web-ui-dependency")
        .arg("file:../../web/nemoir-ui")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target web --web-ui-dependency");

    assert!(output.status.success(), "compile should succeed");
    let pkg = out_dir.join("judge-candidate").join("package.json");
    let pkg_src = std::fs::read_to_string(&pkg).expect("should read package.json");
    assert!(
        pkg_src.contains(r#""@nemoir/web-ui": "file:../../web/nemoir-ui""#),
        "package.json should carry the overridden ui dependency: {pkg_src}"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_ui_dependency_default() {
    let out_dir = std::env::temp_dir().join("nemoir_web_uidefault");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(judge_candidate_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile --target web (default)");

    assert!(output.status.success(), "compile should succeed");
    let pkg = out_dir.join("judge-candidate").join("package.json");
    let pkg_src = std::fs::read_to_string(&pkg).expect("should read package.json");
    assert!(
        pkg_src.contains(r#""@nemoir/web-ui": "^0.2.0""#),
        "package.json should have default ui dependency ^0.2.0: {pkg_src}"
    );

    // main.tsx should import from @nemoir/web-ui
    let main = out_dir.join("judge-candidate").join("src").join("main.tsx");
    let main_src = std::fs::read_to_string(&main).expect("should read main.tsx");
    assert!(
        main_src.contains("@nemoir/web-ui"),
        "main.tsx should import from @nemoir/web-ui: {main_src}"
    );
    assert!(
        main_src.contains("useWebLlmSession"),
        "main.tsx should use useWebLlmSession"
    );
    assert!(
        main_src.contains("useWorkflowRun"),
        "main.tsx should use useWorkflowRun"
    );
    assert!(
        main_src.contains("WebUiHostProvider"),
        "main.tsx should use WebUiHostProvider"
    );
    assert!(
        main_src.contains("ModelLoader"),
        "main.tsx should use ModelLoader"
    );
    assert!(
        main_src.contains("WorkflowTraceDrawer"),
        "main.tsx should use WorkflowTraceDrawer"
    );
    assert!(
        main_src.contains("downloadJsonl"),
        "main.tsx should use downloadJsonl"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_both_dependency_overrides() {
    let out_dir = std::env::temp_dir().join("nemoir_web_bothdep");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(judge_candidate_path())
        .arg("--target")
        .arg("web")
        .arg("--web-runtime-dependency")
        .arg("file:../../web/nemoir-runtime")
        .arg("--web-ui-dependency")
        .arg("file:../../web/nemoir-ui")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile with both overrides");

    assert!(output.status.success(), "compile should succeed");
    let pkg = out_dir.join("judge-candidate").join("package.json");
    let pkg_src = std::fs::read_to_string(&pkg).expect("should read package.json");
    assert!(
        pkg_src.contains(r#""@nemoir/web-runtime": "file:../../web/nemoir-runtime""#),
        "package.json should carry the overridden runtime dependency"
    );
    assert!(
        pkg_src.contains(r#""@nemoir/web-ui": "file:../../web/nemoir-ui""#),
        "package.json should carry the overridden ui dependency"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_hint_tutor_positive() {
    // The Hints-in-Browser-inspired web demo workflow. It uses only
    // user.elicit and string/bool/string[] types, so it must succeed on the
    // web target and produce a hint-tutor/ package.
    let out_dir = std::env::temp_dir().join("nemoir_web_hint_tutor");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(hint_tutor_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile hint_tutor --target web");

    assert!(
        output.status.success(),
        "hint_tutor should compile on web target"
    );

    let pkg = out_dir.join("hint-tutor");
    assert!(
        pkg.join("package.json").exists(),
        "hint-tutor package.json should exist"
    );
    assert!(
        pkg.join("src").join("agent.ts").exists(),
        "hint-tutor src/agent.ts should exist"
    );
    assert!(
        pkg.join("src").join("workflow.json").exists(),
        "hint-tutor src/workflow.json should exist"
    );

    let wf_json = std::fs::read_to_string(pkg.join("src").join("workflow.json"))
        .expect("should read workflow.json");
    assert!(
        wf_json.contains("HintTutor"),
        "workflow.json should contain workflow id"
    );
    assert!(
        wf_json.contains("user.elicit"),
        "workflow.json should carry the user.elicit capability"
    );

    let agent =
        std::fs::read_to_string(pkg.join("src").join("agent.ts")).expect("should read agent.ts");
    assert!(
        agent.contains("learner_code: string"),
        "agent.ts should have typed learner_code input"
    );
    assert!(
        agent.contains("key_points: string[]"),
        "agent.ts should have typed key_points output"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_http_fetch_positive() {
    let out_dir = std::env::temp_dir().join("nemoir_web_http_fetch");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(http_fetch_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile http_fetch --target web");

    assert!(
        output.status.success(),
        "http_fetch should compile on web target"
    );

    let pkg = out_dir.join("http-fetch-demo");
    assert!(
        pkg.join("package.json").exists(),
        "http-fetch-demo package.json should exist"
    );
    assert!(
        pkg.join("src").join("agent.ts").exists(),
        "http-fetch-demo src/agent.ts should exist"
    );
    assert!(
        pkg.join("src").join("workflow.json").exists(),
        "http-fetch-demo src/workflow.json should exist"
    );

    let wf_json = std::fs::read_to_string(pkg.join("src").join("workflow.json"))
        .expect("should read workflow.json");
    assert!(
        wf_json.contains("HttpFetchDemo"),
        "workflow.json should contain workflow id"
    );
    assert!(
        wf_json.contains("http.fetch"),
        "workflow.json should carry the http.fetch capability"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_js_run_model_stage_negative() {
    let out_dir = std::env::temp_dir().join("nemoir_web_js_run_neg");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(js_run_model_stage_err_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile js_run_model_stage_err --target web");

    assert!(
        !output.status.success(),
        "browser.js.run in model-stage requires must fail on web target"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("browser.js.run"),
        "should mention browser.js.run: {stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("deterministic"),
        "should mention deterministic-only: {stderr}"
    );

    // No output directory should have been created.
    assert!(
        !out_dir.join("js-run-model-err").exists(),
        "web backend must not write files on validation failure"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_js_run_positive() {
    // Medium #3: a positive compile + codegen test for `browser.js.run`.
    // Verifies the generated app carries the js.worker.ts asset and
    // HAS_JS_RUN plumbing (only present when the workflow uses js.run).
    let out_dir = std::env::temp_dir().join("nemoir_web_js_run_pos");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(js_run_positive_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile js_run_positive --target web");

    assert!(
        output.status.success(),
        "js_run_positive should compile on web target"
    );

    let pkg = out_dir.join("js-run-positive");
    assert!(
        pkg.join("src").join("js.worker.ts").exists(),
        "js-run-positive should emit src/js.worker.ts (HAS_JS_RUN path)"
    );
    let agent_ts =
        std::fs::read_to_string(pkg.join("src").join("agent.ts")).expect("should read agent.ts");
    assert!(
        agent_ts.contains("HAS_JS_RUN = true"),
        "agent.ts should declare HAS_JS_RUN = true: {agent_ts}"
    );
    assert!(
        agent_ts.contains("browser.js.run"),
        "agent.ts REQUIRED_CAPABILITIES should list browser.js.run: {agent_ts}"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_js_sandbox_positive() {
    let out_dir = std::env::temp_dir().join("nemoir_web_js_sandbox_pos");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(js_sandbox_positive_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile js_sandbox_positive --target web");

    assert!(
        output.status.success(),
        "js_sandbox_positive should compile on web target: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let pkg = out_dir.join("js-sandbox-positive");
    assert!(
        !pkg.join("src").join("js.worker.ts").exists(),
        "dynamic sandbox must not reuse the trusted same-origin js.worker.ts"
    );
    let agent_ts =
        std::fs::read_to_string(pkg.join("src").join("agent.ts")).expect("should read agent.ts");
    assert!(
        agent_ts.contains("HAS_JS_SANDBOX = true"),
        "agent.ts should declare HAS_JS_SANDBOX = true: {agent_ts}"
    );
    assert!(
        agent_ts.contains("HAS_JS_RUN = false"),
        "agent.ts should keep trusted HAS_JS_RUN false: {agent_ts}"
    );
    let main_tsx =
        std::fs::read_to_string(pkg.join("src").join("main.tsx")).expect("should read main.tsx");
    assert!(main_tsx.contains("createOpaqueOriginJsSandbox"));
    assert!(main_tsx.contains("jsSandboxRunner"));
    let workflow = std::fs::read_to_string(pkg.join("src").join("workflow.json"))
        .expect("should read workflow.json");
    assert!(workflow.contains("browser.js.sandbox"));
    assert!(workflow.contains("before browser.js.sandbox(code) requires user.confirm"));

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_js_sandbox_user_code_is_model_free() {
    let out_dir = std::env::temp_dir().join("nemoir_web_js_sandbox_user");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(js_sandbox_user_code_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should compile user-code sandbox workflow");
    assert!(
        output.status.success(),
        "user-code sandbox workflow should compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let pkg = out_dir.join("js-sandbox-user-code");
    let agent_ts =
        std::fs::read_to_string(pkg.join("src").join("agent.ts")).expect("should read agent.ts");
    assert!(agent_ts.contains("HAS_MODEL_STAGES = false"));
    assert!(agent_ts.contains("HAS_JS_SANDBOX = true"));

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_compile_web_js_sandbox_json_code_type_rejected() {
    // The DSL lowers successfully; the web-target capability contract must
    // reject browser.js.sandbox `code` that resolves to a non-string type.
    let out_dir = std::env::temp_dir().join("nemoir_web_js_sandbox_json_type");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("should create tempdir");

    let output = nemoir_binary()
        .arg("compile")
        .arg(js_sandbox_code_json_type_err_path())
        .arg("--target")
        .arg("web")
        .arg("-o")
        .arg(&out_dir)
        .output()
        .expect("should run nemo compile js_sandbox_code_json_type_err --target web");

    assert!(
        !output.status.success(),
        "json-typed code input should fail on the web target"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("non-optional string"),
        "should mention non-optional string: {stderr}"
    );
    assert!(
        !out_dir.join("js-sandbox-code-json-type").exists(),
        "web backend must not write files on validation failure"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}
