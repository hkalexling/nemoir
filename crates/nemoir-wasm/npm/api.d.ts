/**
 * @nemoir/compiler-wasm — TypeScript declarations
 *
 * Hand-authored declarations for the NemoIR compiler WASM package.
 * These replace wasm-bindgen's generated `any` types with the actual
 * contract the Rust serde shapes emit via `json_compatible()` serialization.
 *
 * Canonical Rust types live in `crates/nemoir-wasm/src/api.rs` in the
 * https://github.com/hkalexling/nemoir compiler repository.
 */

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/** WebAssembly module sync input (bytes or precompiled module). */
export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Synchronously initialize the WASM module from raw bytes or a precompiled
 * `WebAssembly.Module`.  Use this in Workers when you already hold the bytes
 * and want to avoid a `fetch` round-trip.
 */
export function initSync(
  module: { module: SyncInitInput } | SyncInitInput,
): InitOutput;

/**
 * Default async initialization.  When called with no arguments, fetches
 * `nemoir_wasm_bg.wasm` relative to the importing module. Pass a
 * `Response`, `URL`, `Request`, `Promise<Response>`, or raw bytes to
 * override.
 */
export default function init(
  module_or_path?:
    | { module_or_path: InitInput | Promise<InitInput> }
    | InitInput
    | Promise<InitInput>,
): Promise<InitOutput>;

export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly analyze: (a: any) => [number, number, number];
  readonly generate: (a: any) => [number, number, number];
  readonly metadata: () => [number, number, number];
}

// ---------------------------------------------------------------------------
// Domain types (mirrors api.rs serde shapes)
// ---------------------------------------------------------------------------

export type Target = "none" | "visualizer" | "python" | "web";

export interface CompileRequest {
  /** LF-normalized `.nemo` source text. */
  source: string;
  /** Display name only — never a host filesystem path. */
  filename?: string;
  /** Backend target for code generation. */
  target?: Target;
  /** When `true`, include the lowered WorkflowIr JSON in the response. */
  includeIr?: boolean;
  /** Reserved for documented backend options. */
  options?: Record<string, never>;
}

export interface SourcePosition {
  /** 1-based line number. */
  line: number;
  /** 1-based UTF-16 column offset. */
  utf16Column: number;
}

export interface SourceRange {
  start: SourcePosition;
  end: SourcePosition;
}

export type DiagnosticPhase = "dsl" | "ir" | "target" | "internal";

export interface CompilerDiagnostic {
  phase: DiagnosticPhase;
  severity: "error";
  message: string;
  help?: string;
  code?: string;
  range?: SourceRange;
}

export interface ArtifactFile {
  /** Safe UTF-8 path relative to `Artifact.archiveRoot`. */
  path: string;
  /** UTF-8 file content. */
  content: string;
}

export interface Artifact {
  target: Target;
  packageName: string;
  archiveRoot: string;
  files: ArtifactFile[];
}

export interface AnalyzeResponse {
  ok: boolean;
  compilerVersion: string;
  irVersion?: string;
  ir?: unknown;
  diagnostics: CompilerDiagnostic[];
}

export interface GenerateResponse {
  ok: boolean;
  compilerVersion: string;
  irVersion?: string;
  ir?: unknown;
  diagnostics: CompilerDiagnostic[];
  artifact?: Artifact;
}

export interface CompilerMetadata {
  compilerVersion: string;
  irVersion: string;
  supportedTargets: string[];
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Analyse `.nemo` source: parse, lower, validate, and optionally return IR.
 * Called on a debounce while the user edits.
 */
export function analyze(request: CompileRequest): AnalyzeResponse;

/**
 * Generate a download artifact from valid `.nemo` source.
 * Called only on a deliberate user action (e.g. "Download ZIP").
 */
export function generate(request: CompileRequest): GenerateResponse;

/**
 * Return compiler and IR version metadata for the about / debug view.
 */
export function metadata(): CompilerMetadata;
