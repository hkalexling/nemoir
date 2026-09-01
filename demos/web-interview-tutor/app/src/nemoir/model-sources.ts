/**
 * Deployer-controlled model-source profiles for the interview tutor.
 *
 * This is the controlled-mirror seam. A deployer who hosts their own
 * mirrored MLC artifacts (institutional CDN, Cloudflare R2, S3-compatible
 * object storage, etc.) declares them here as `ModelSourceProfile`s. The
 * session hook overlays them onto WebLLM's prebuilt Hugging Face records so
 * the model picker offers `Model-MLC@source` entries that download from the
 * mirror URLs instead of huggingface.co.
 *
 * Requirements for a mirror origin:
 * - Correct CORS/CORP headers for this app's COEP deployment.
 * - Correct `application/wasm` MIME type for the `.wasm` library.
 * - Immutable, versioned paths so cache entries never mix across releases.
 * - The exact MLC artifact layout (weights + mlc-chat-config.json + tokenizer
 *   files) and the matching WebGPU WASM library for this `@mlc-ai/web-llm`
 *   version (`modelVersion = "v0_2_84/base"`)
 *
 * Source-specific model IDs (`…-MLC@source`) ensure a mirror and the upstream
 * HF record never share a browser cache entry — a mixed cache from two
 * different origins is a common source of corruption.
 *
 * To enable a mirror, add it to `MODEL_SOURCE_PROFILES` below.
 */

import type { ModelSourceProfile } from "@nemoir/web-ui";

/**
 * Declared mirror profiles. Empty by default; a deployer fills this in with
 * their own hosted artifacts. Each entry becomes a set of
 * `Model-MLC@<sourceId>` options in the model picker.
 */
export const MODEL_SOURCE_PROFILES: readonly ModelSourceProfile[] = [];

/**
 * Build the `extraModels` list for `createWebllmSession` / `useWebLlmSession`
 * from the declared profiles. Currently returns an empty array (no mirrors
 * configured), but the overlay logic is exercised so adding a profile is a
 * single-file change.
 */
export function buildMirrorModelRecords(): readonly ModelSourceProfile[] {
  return MODEL_SOURCE_PROFILES;
}
