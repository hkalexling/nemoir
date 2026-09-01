// Auto-generated static worker for browser.js.run.
// Receives `{ code, input }` via postMessage and executes the trusted code.
self.onmessage = async (e: MessageEvent) => {
  const { code, input } = e.data as { code: string; input: unknown };
  try {
    const fn = new Function("input", code);
    const result = await fn(input);
    self.postMessage(result);
  } catch (err) {
    self.postMessage({
      __error: err instanceof Error ? err.message : String(err),
    });
  }
};
