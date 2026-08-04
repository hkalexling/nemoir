//! IR-dependent file translation: emits `workflow.json` (the canonical
//! serialized IR) and `agent.ts` (a typed Agent facade).
//!
//! Unlike the Python backend's `translate.rs` (~1400 lines of IR → Python
//! dataclass-literal codegen), the web backend emits the IR directly as
//! JSON and generates only a thin typed facade, leaving runtime semantics
//! to `@nemoir/web-runtime`.

use std::path::PathBuf;

use nemoir_ir::WorkflowIr;

use crate::escape::ts_template_body;
use crate::naming::is_valid_ts_identifier;
use crate::{GeneratedFile, WebBackendError};

/// Emit `src/workflow.json` — the canonical serialized IR.
///
/// The IR types are `serde::Serialize`, so this is a direct JSON dump.
/// `serde_json` with `preserve_order` keeps key ordering stable for
/// inspectability.
pub fn emit_workflow_json(ir: &WorkflowIr) -> Result<String, WebBackendError> {
    serde_json::to_string_pretty(ir).map_err(|e| WebBackendError::JsonSerialization(e.to_string()))
}

/// Check whether an IR contains any trusted `browser.js.run` exec stage.
pub fn has_js_run_stage(ir: &nemoir_ir::WorkflowIr) -> bool {
    ir.nodes.iter().any(|n| {
        matches!(&n.execution, nemoir_ir::StageExecution::Tool { capability, .. } if capability == "browser.js.run")
    })
}

/// Check whether an IR contains any dynamic `browser.js.sandbox` exec stage.
pub fn has_js_sandbox_stage(ir: &nemoir_ir::WorkflowIr) -> bool {
    ir.nodes.iter().any(|n| {
        matches!(&n.execution, nemoir_ir::StageExecution::Tool { capability, .. } if capability == "browser.js.sandbox")
    })
}

/// Map an IR type string to a TypeScript type.
fn ir_type_to_ts(ty: &str) -> &'static str {
    match ty {
        "string" => "string",
        "bool" => "boolean",
        "number" => "number",
        "json" => "unknown",
        // `path` is rejected by validate_for_web, but map defensively.
        "path" => "string",
        "string[]" => "string[]",
        _ => {
            // Defensive: unknown types (e.g. a hypothetical "bool[]") get
            // a permissive mapping. validate_for_web already rejects path.
            if ty.ends_with("[]") {
                "unknown[]"
            } else {
                "unknown"
            }
        }
    }
}

/// Collect `(name, ts_type, is_optional)` for every write across all
/// exit stages, preserving declaration order without duplicating names.
/// Mirrors `nemoir-backend-python::collect_exit_fields`.
fn collect_exit_fields(
    ir: &WorkflowIr,
) -> Result<Vec<(String, &'static str, bool)>, WebBackendError> {
    let exit_set: std::collections::HashSet<&str> =
        ir.workflow.exits.iter().map(|s| s.as_str()).collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<(String, &'static str, bool)> = Vec::new();
    for node in &ir.nodes {
        if !exit_set.contains(node.id.as_str()) {
            continue;
        }
        for w in &node.writes {
            if !is_valid_ts_identifier(&w.name) {
                return Err(WebBackendError::InvalidWebField(w.name.clone()));
            }
            if seen.insert(w.name.clone()) {
                out.push((w.name.clone(), ir_type_to_ts(&w.ty), w.optional));
            }
        }
    }
    Ok(out)
}

/// Render a TS property type for an exit write.
/// - multi-exit: every field is optional `T | null`
/// - single-exit required: `T`
/// - single-exit optional: `T | null`
fn ts_output_field_type(ts_type: &str, optional: bool, multi_exit: bool) -> String {
    if multi_exit || optional {
        format!("{ts_type} | null")
    } else {
        ts_type.to_string()
    }
}

/// Emit `src/agent.ts` — a typed Agent facade.
///
/// Generates `AgentInput`, `AgentOutput`, `AgentResult` interfaces from the
/// IR inputs and exit-stage writes, plus an `Agent` class scaffold with
/// `run()` / `stream()` method signatures. The runtime implementation is
/// wired in Phase 2+.
pub fn emit_agent_ts(ir: &WorkflowIr) -> Result<String, WebBackendError> {
    let package_dir = crate::naming::package_dir(&ir.workflow.id)
        .ok_or_else(|| WebBackendError::InvalidWorkflowId(ir.workflow.id.clone()))?;

    // Validate input ids are valid TS identifiers
    for inp in &ir.inputs {
        if !is_valid_ts_identifier(&inp.id) {
            return Err(WebBackendError::InvalidWebField(inp.id.clone()));
        }
        if ir_type_to_ts(&inp.ty) == "string" && inp.ty.contains("path") {
            // Defensive: validate_for_web should have caught this already.
            return Err(WebBackendError::UnsupportedForWebTarget(format!(
                "input \"{}\" has path type which is unsupported on web",
                inp.id
            )));
        }
    }

    let exit_fields = collect_exit_fields(ir)?;
    let multi_exit = ir.workflow.exits.len() > 1;

    let workflow_id_escaped = ts_template_body(&ir.workflow.id);
    let entry_escaped = ts_template_body(&ir.workflow.entry);

    // AgentInput interface
    let input_fields = if ir.inputs.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        for inp in &ir.inputs {
            let ts = ir_type_to_ts(&inp.ty);
            // Optional inputs become `T | null` (the runtime treats missing
            // optional inputs as null, matching the Python runtime's optional
            // read semantics).
            if inp.ty.ends_with('?') || inp.ty.ends_with("[]?") {
                s.push_str(&format!("  {}: {} | null;\n", inp.id, ts));
            } else {
                s.push_str(&format!("  {}: {};\n", inp.id, ts));
            }
        }
        s
    };

    // AgentOutput interface
    let output_fields = if exit_fields.is_empty() {
        String::new()
    } else {
        let mut s = String::new();
        for (name, ts_type, optional) in &exit_fields {
            let ty_str = ts_output_field_type(ts_type, *optional, multi_exit);
            if multi_exit || *optional {
                s.push_str(&format!("  {}?: {};\n", name, ty_str));
            } else {
                s.push_str(&format!("  {}: {};\n", name, ty_str));
            }
        }
        s
    };

    // Exit stage ids as a const tuple
    let exits: Vec<String> = ir
        .workflow
        .exits
        .iter()
        .map(|e| format!("\"{}\"", e))
        .collect();
    let exits_str = exits.join(", ");

    // Detect whether the workflow contains any model stages.
    let has_model_stages = ir
        .nodes
        .iter()
        .any(|n| matches!(n.execution, nemoir_ir::StageExecution::Model));
    let has_model_stages_str = if has_model_stages { "true" } else { "false" };

    // Detect trusted and dynamic JavaScript execution separately. The former
    // needs an emitted same-origin Worker; the latter wires the runtime's
    // opaque-origin iframe sandbox only when the IR declares it.
    let has_js_run = has_js_run_stage(ir);
    let has_js_run_str = if has_js_run { "true" } else { "false" };
    let has_js_sandbox = has_js_sandbox_stage(ir);
    let has_js_sandbox_str = if has_js_sandbox { "true" } else { "false" };

    // Required capabilities
    let caps: Vec<String> = ir
        .capabilities
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect();
    let caps_str = caps.join(", ");

    let agent_src = format!(
        r#"// Auto-generated by `nemo compile --target web`. Do not edit.
// Workflow: {workflow_id}
//
// This module is a typed facade over @nemoir/web-runtime. The compiled
// workflow IR is embedded as `./workflow.json` (inspectable JSON). The
// Agent class delegates to WorkflowAgent from the runtime and exposes
// typed run()/stream() methods.
//
// The runtime (@nemoir/web-runtime) is framework-neutral: it does not
// require React. The generated main.tsx is one possible consumer.

import {{
  WorkflowAgent,
  type BrowserToolsOptions,
  type RunOptions,
  type WorkflowEvent,
  type ModelAdapter,
  type ModelRouter,
  type Tool,
  type ToolRegistry,
  type WebUiHost,
}} from "@nemoir/web-runtime";
import workflowManifest from "./workflow.json";

export const WORKFLOW_ID = `{workflow_id_escaped}` as const;
export const ENTRY_STAGE_ID = `{entry_escaped}` as const;
export const EXIT_STAGE_IDS = [{exits_str}] as const;
export const REQUIRED_CAPABILITIES = [{caps_str}] as const;
export const HAS_MODEL_STAGES = {has_model_stages_str} as const;
export const HAS_JS_RUN = {has_js_run_str} as const;
export const HAS_JS_SANDBOX = {has_js_sandbox_str} as const;

export interface AgentInput {{
{input_fields}}}

export interface AgentOutput {{
{output_fields}}}

export interface AgentResult {{
  output: AgentOutput;
}}

export interface AgentOptions {{
  /**
   * Model adapter. Required for workflows with model stages; optional for
   * deterministic-only workflows. Pass a WebLLM adapter via
   * `createWebllmAdapter`, or inject a fake adapter for tests.
   */
  modelAdapter?: ModelAdapter | ModelRouter;
  /** Tool registry. Merged with built-in UI-host tools. */
  tools?: ToolRegistry | Iterable<Tool>;
  /** UI host for browser-safe capabilities (user.elicit / user.confirm). */
  uiHost?: WebUiHost;
  /**
   * Options for browser-native tools (http.fetch, browser.storage.*,
   * browser.js.run, browser.js.sandbox). Pass `jsWorkerFactory` for trusted
   * browser.js.run stages or `jsSandboxRunner` for dynamic sandbox stages.
   */
  browserTools?: BrowserToolsOptions;
  /** Default run options (can be overridden per run). */
  defaults?: Partial<RunOptions>;
  /**
   * Model action protocol. The web baseline is the tagged-envelope protocol
   * (works with small local WebLLM models that lack reliable function
   * calling). Override with `"native"` only for models you know support
   * OpenAI-style tool calls.
   */
  actionProtocol?: "native" | "tagged_envelope";
}}

export interface AgentRunOptions extends Partial<RunOptions> {{
  /** Override the per-run action protocol. */
  actionProtocol?: "native" | "tagged_envelope";
}}

export class Agent {{
  readonly workflowId = WORKFLOW_ID;
  readonly requiredCapabilities = REQUIRED_CAPABILITIES;
  readonly manifest = workflowManifest;

  constructor(private readonly opts: AgentOptions) {{}}

  private createAgent(actionProtocol?: "native" | "tagged_envelope"): WorkflowAgent {{
    if (HAS_MODEL_STAGES && !this.opts.modelAdapter) {{
      throw new Error(
        "Agent requires a modelAdapter. Pass a WebLLM adapter (`createWebllmAdapter` " +
        "from `./webllm`) or another ModelAdapter via AgentOptions.",
      );
    }}
    return new WorkflowAgent(workflowManifest, {{
      modelAdapter: this.opts.modelAdapter,
      tools: this.opts.tools,
      uiHost: this.opts.uiHost,
      browserTools: this.opts.browserTools,
      defaults: this.opts.defaults,
      actionProtocol: actionProtocol ?? this.opts.actionProtocol ?? "tagged_envelope",
    }});
  }}

  async run(inputs: AgentInput, options?: AgentRunOptions): Promise<AgentResult> {{
    const agent = this.createAgent(options?.actionProtocol);
    const {{ actionProtocol: _ap, ...runOpts }} = options ?? {{}};
    const result = await agent.run(inputs as unknown as Record<string, unknown>, {{ options: runOpts }});
    return {{ output: result.output as unknown as AgentOutput }};
  }}

  async *stream(inputs: AgentInput, options?: AgentRunOptions): AsyncIterable<WorkflowEvent> {{
    const agent = this.createAgent(options?.actionProtocol);
    const {{ actionProtocol: _ap, ...runOpts }} = options ?? {{}};
    yield* agent.stream(inputs as unknown as Record<string, unknown>, {{ options: runOpts }});
  }}
}}
"#,
        workflow_id = ir.workflow.id,
        workflow_id_escaped = workflow_id_escaped,
        entry_escaped = entry_escaped,
        exits_str = exits_str,
        caps_str = caps_str,
        has_model_stages_str = has_model_stages_str,
        has_js_run_str = has_js_run_str,
        has_js_sandbox_str = has_js_sandbox_str,
        input_fields = input_fields,
        output_fields = output_fields,
    );

    // package_dir is used to validate the workflow id is convertible; the
    // directory name is the caller's concern (lib.rs derives it).
    let _ = package_dir;

    Ok(agent_src)
}

/// Build the full list of generated files for a web package.
///
/// Returns files with `relative_path` relative to the output directory
/// (i.e. prefixed with the package dir name).
pub fn build_files(
    ir: &WorkflowIr,
    package_dir: &str,
    version: &str,
    runtime_dep: &str,
    ui_dep: &str,
) -> Result<Vec<GeneratedFile>, WebBackendError> {
    let workflow_json = emit_workflow_json(ir)?;
    let agent_ts = emit_agent_ts(ir)?;
    let has_js_run = has_js_run_stage(ir);
    let has_js_sandbox = has_js_sandbox_stage(ir);
    let main_tsx = crate::emit::emit_main_tsx(has_js_run, has_js_sandbox);
    let app_css = crate::emit::emit_app_css();

    let mut files = vec![
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/package.json")),
            content: crate::emit::emit_package_json(package_dir, version, runtime_dep, ui_dep),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/tsconfig.json")),
            content: crate::emit::emit_tsconfig_json(),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/tsconfig.node.json")),
            content: crate::emit::emit_tsconfig_node_json(),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/vite.config.ts")),
            content: crate::emit::emit_vite_config(),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/index.html")),
            content: crate::emit::emit_index_html(package_dir, &ir.workflow.id),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/netlify.toml")),
            content: crate::emit::emit_netlify_toml(),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/vercel.json")),
            content: crate::emit::emit_vercel_json(),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/public/_headers")),
            content: crate::emit::emit_public_headers(),
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/src/workflow.json")),
            content: workflow_json,
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/src/agent.ts")),
            content: agent_ts,
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/src/main.tsx")),
            content: main_tsx,
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/src/app.css")),
            content: app_css,
        },
        GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/src/webllm.worker.ts")),
            content: crate::emit::emit_webllm_worker(),
        },
    ];

    // Only emit (and import) the browser.js.run worker when the workflow
    // actually uses `browser.js.run`. Otherwise the generated app would ship
    // a worker asset that no code references (and a `new Worker(new URL(...))`
    // expression that Vite statically analyzes would fail to resolve).
    if has_js_run {
        files.push(GeneratedFile {
            relative_path: PathBuf::from(format!("{package_dir}/src/js.worker.ts")),
            content: crate::emit::emit_js_run_worker(),
        });
    }

    files.push(GeneratedFile {
        relative_path: PathBuf::from(format!("{package_dir}/.gitignore")),
        content: crate::emit::emit_gitignore(),
    });
    files.push(GeneratedFile {
        relative_path: PathBuf::from(format!("{package_dir}/README.md")),
        content: crate::emit::emit_readme(&ir.workflow.id, package_dir, &ir.workflow.entry),
    });

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judge_candidate_ir() -> WorkflowIr {
        let source = include_str!("../../nemoir-dsl-fe/tests/fixtures/judge_candidate-ir.yml");
        serde_yaml::from_str(source).expect("should parse judge_candidate-ir.yml")
    }

    #[test]
    fn workflow_json_is_valid_json_and_round_trips() {
        let ir = judge_candidate_ir();
        let json = emit_workflow_json(&ir).unwrap();
        // Must parse as JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("workflow.json must be valid JSON");
        // Round-trip back into WorkflowIr (semantic equality)
        let back: WorkflowIr =
            serde_json::from_value(parsed).expect("workflow.json must round-trip to WorkflowIr");
        assert_eq!(back, ir, "workflow.json must round-trip semantically");
    }

    #[test]
    fn agent_ts_has_typed_input_and_output() {
        let ir = judge_candidate_ir();
        let src = emit_agent_ts(&ir).unwrap();
        assert!(src.contains("export interface AgentInput {"));
        // judge_candidate has input eps: number
        assert!(src.contains("eps: number"));
        assert!(src.contains("export interface AgentOutput {"));
        assert!(src.contains("export interface AgentResult {"));
        assert!(src.contains("export class Agent {"));
        assert!(src.contains("WORKFLOW_ID"));
        assert!(src.contains("JudgeCandidate"));
    }

    #[test]
    fn build_files_produces_expected_layout() {
        let ir = judge_candidate_ir();
        let files = build_files(&ir, "judge-candidate", "0.1.0", "^0.1.0", "^0.1.0").unwrap();
        let paths: Vec<String> = files
            .iter()
            .map(|f| f.relative_path.to_string_lossy().into_owned())
            .collect();
        assert!(
            paths.iter().any(|p| p == "judge-candidate/package.json"),
            "missing package.json: {paths:?}"
        );
        assert!(
            paths
                .iter()
                .any(|p| p == "judge-candidate/src/workflow.json"),
            "missing workflow.json: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p == "judge-candidate/src/agent.ts"),
            "missing agent.ts: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p == "judge-candidate/netlify.toml"),
            "missing netlify.toml: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p == "judge-candidate/public/_headers"),
            "missing public/_headers: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p == "judge-candidate/vite.config.ts"),
            "missing vite.config.ts: {paths:?}"
        );
    }
}
