import { WebWorkerMLCEngineHandler } from "@mlc-ai/web-llm";

// The handler resides in the worker thread and processes all model
// computation off the main UI thread. The main thread holds a
// WebWorkerMLCEngine proxy that sends messages here.
const handler = new WebWorkerMLCEngineHandler();
self.onmessage = (msg: MessageEvent) => {
  handler.onmessage(msg);
};
