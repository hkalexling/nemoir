/**
 * Tutor-local adapter around the shared WebLLM lifecycle hook.
 *
 * The shared package owns session creation, model cache assessment, download
 * progress, and disposal. This module supplies the generated tutor worker and
 * derives a small presentation status without hiding the controls App needs
 * for model selection/loading.
 */

import {
  assessModelFit,
  probeDeviceCapabilities,
  toMirroredModelRecord,
  useWebLlmSession as useSharedSession,
  type UseWebLlmSessionResult,
  type WebLlmLoadFailure,
  type ModelFitAssessment,
  type DeviceCapabilityReport,
} from "@nemoir/web-ui";
import { useEffect, useMemo, useState } from "react";
import { MODEL_SOURCE_PROFILES } from "./model-sources.js";

interface SessionEnvironment {
  readonly webGpuAvailable: boolean;
  readonly crossOriginIsolated: boolean;
}

export type WebLlmSessionStatus =
  | (SessionEnvironment & {
      readonly kind: "idle";
      readonly message: string;
    })
  | (SessionEnvironment & {
      readonly kind: "unavailable";
      readonly message: string;
    })
  | (SessionEnvironment & {
      readonly kind: "starting";
      readonly message: string;
    })
  | (SessionEnvironment & {
      readonly kind: "ready";
      readonly message: string;
      readonly modelCount: number;
      readonly selectedModel: string;
      readonly modelLoaded: boolean;
      readonly loading: boolean;
    })
  | (SessionEnvironment & {
      readonly kind: "error";
      readonly message: string;
      readonly loadFailure: WebLlmLoadFailure | null;
    });

/** Full shared lifecycle state plus a tutor-specific display status. */
export type TutorWebLlmSession = UseWebLlmSessionResult & {
  readonly enabled: boolean;
  readonly status: WebLlmSessionStatus;
  /** Models sorted by device fit (recommended first). */
  readonly sortedModels: readonly import("@nemoir/web-ui").WebLlmModelInfo[];
  /** Per-model fit assessment keyed by modelId, or null while unresolved. */
  readonly fitAssessments: ReadonlyMap<string, ModelFitAssessment> | null;
  /** Probed device capability report, or null while unresolved. */
  readonly device: DeviceCapabilityReport | null;
};

/**
 * Start a local WebLLM session only after the learner explicitly opens local
 * tutor setup. A deterministic test-only visit keeps this disabled, so it
 * neither probes WebGPU nor creates a model worker.
 */
export function useWebLlmSession(enabled = false): TutorWebLlmSession {
  // Build mirror ModelRecords from the deployer-controlled source profiles.
  // These become `Model-MLC@source` entries in the model picker, downloading
  // from a controlled CDN instead of huggingface.co. Empty by default.
  const mirrorRecords = useMemo(
    () =>
      MODEL_SOURCE_PROFILES.flatMap((profile) =>
        profile.models.map((mirror) => toMirroredModelRecord(mirror, profile)),
      ),
    [],
  );

  const state = useSharedSession({
    enabled,
    workerFactory: () =>
      new Worker(
        new URL(
          "../generated/interview-tutor/src/webllm.worker.ts",
          import.meta.url,
        ),
        { type: "module" },
      ),
    extraModels: mirrorRecords.length > 0 ? mirrorRecords : undefined,
  });

  const [device, setDevice] = useState<DeviceCapabilityReport | null>(null);

  // Probe WebGPU device capabilities once, when the learner enables local
  // tutor setup. This is a one-shot read; adapters rarely change during a
  // session.
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    probeDeviceCapabilities()
      .then((report) => {
        if (!cancelled) setDevice(report);
      })
      .catch(() => {
        if (!cancelled)
          setDevice({
            webgpu: {
              available: true,
              adapterInfo: null,
              shaderF16Supported: null,
              maxStorageBufferBindingSize: null,
              probeError: true,
            },
            storage: { supported: false, quota: null, usage: null, available: null },
          });
      });
    return () => {
      cancelled = true;
    };
  }, [enabled]);

  // Classify every model against the probed device so the UI can present
  // recommended models first and annotate the rest.
  const fitAssessments = useMemo<ReadonlyMap<string, ModelFitAssessment> | null>(() => {
    if (!device) return null;
    const cached = new Set(state.cachedIds ?? []);
    const map = new Map<string, ModelFitAssessment>();
    for (const model of state.models) {
      map.set(
        model.modelId,
        assessModelFit(model, device, {
          isCached: cached.has(model.modelId),
          estimatedDownloadBytes: model.estimatedDownloadBytes,
        }),
      );
    }
    return map;
  }, [device, state.cachedIds, state.models]);

  // Sort models: recommended/likely_ok first (by VRAM asc), then everything
  // else by VRAM asc. Hard blockers last so the learner doesn't default to
  // an unusable model.
  const sortedModels = useMemo<readonly import("@nemoir/web-ui").WebLlmModelInfo[]>(() => {
    const priority: Record<string, number> = {
      recommended: 0,
      likely_ok: 1,
      needs_download: 2,
      unknown: 3,
      oversized_vram: 4,
      buffer_limit: 5,
      missing_feature: 6,
    };
    const models = [...state.models];
    return models.sort((a, b) => {
      const pa = fitAssessments?.get(a.modelId);
      const pb = fitAssessments?.get(b.modelId);
      const catA = pa ? (priority[pa.category] ?? 3) : 3;
      const catB = pb ? (priority[pb.category] ?? 3) : 3;
      if (catA !== catB) return catA - catB;
      return (a.vramRequiredMb ?? 0) - (b.vramRequiredMb ?? 0);
    });
  }, [fitAssessments, state.models]);

  const status = useMemo((): WebLlmSessionStatus => {
    const env: SessionEnvironment = {
      webGpuAvailable: state.webgpuAvailable,
      crossOriginIsolated: state.crossOriginIsolated,
    };

    if (!enabled) {
      return {
        ...env,
        kind: "idle",
        message: "Enable the local tutor when you are ready to select a WebLLM model.",
      };
    }
    if (!env.webGpuAvailable) {
      return {
        ...env,
        kind: "unavailable",
        message: "WebGPU is unavailable; local AI guidance cannot run in this browser.",
      };
    }
    if (!env.crossOriginIsolated) {
      return {
        ...env,
        kind: "unavailable",
        message: "Cross-origin isolation is inactive; verify the configured COOP/COEP headers.",
      };
    }
    if (state.error) {
      return { ...env, kind: "error", message: state.error, loadFailure: state.loadFailure };
    }
    if (!state.session) {
      return {
        ...env,
        kind: "starting",
        message: "Starting the local WebLLM model catalog…",
      };
    }

    const modelLoaded = state.isModelLoaded;
    const message = state.loading
      ? "Downloading and preparing the selected local model…"
      : modelLoaded
        ? "The selected local model is ready for evidence-backed tutoring."
        : "Choose a model and load it before requesting a local hint.";

    return {
      ...env,
      kind: "ready",
      message,
      modelCount: state.models.length,
      selectedModel: state.selectedModel,
      modelLoaded,
      loading: state.loading,
    };
  }, [
    enabled,
    state.crossOriginIsolated,
    state.error,
    state.isModelLoaded,
    state.loading,
    state.models.length,
    state.selectedModel,
    state.session,
    state.webgpuAvailable,
  ]);

  return { ...state, enabled, status, sortedModels, fitAssessments, device };
}
