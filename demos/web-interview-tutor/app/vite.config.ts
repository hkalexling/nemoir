import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

// WebLLM requires cross-origin isolation for SharedArrayBuffer support.
const crossOriginIsolationHeaders = {
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Resource-Policy": "cross-origin",
};

// GitHub Pages serves project sites below `/<repository>/`. The Pages workflow
// supplies that path through VITE_BASE_PATH; local and headers-capable hosts
// retain the root base URL.
function normalizedBasePath(value: string | undefined): string {
  if (!value || value === "/") return "/";
  return `/${value.replace(/^\/+|\/+$/g, "")}/`;
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, ".", "VITE_");

  return {
    base: normalizedBasePath(env.VITE_BASE_PATH),
    plugins: [react()],
    worker: {
      format: "es",
    },
    // Monaco imports ESM workers via `?worker`; pre-bundling it would turn
    // those worker URLs into missing optimize-deps artifacts in development.
    optimizeDeps: {
      exclude: ["monaco-editor"],
    },
    server: {
      allowedHosts: true,
      headers: crossOriginIsolationHeaders,
    },
    preview: {
      headers: crossOriginIsolationHeaders,
    },
  };
});
