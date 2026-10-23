// Each job owns an isolated Wasm instance. Yield actual tasks so Stop can be
// handled between chunks; Promise microtasks alone would starve messages.
import init, { WorkerSearch } from "./plassi.js";

let search;
let stopped = false;
let lastProgress = 0;

function step() {
  try {
    if (stopped) search.stop();
    const outcome = search.advance(20);
    if (outcome !== undefined) {
      self.postMessage({ type: "done", data: outcome });
      search.free();
      search = undefined;
      return;
    }
    if (performance.now() - lastProgress >= 100) {
      lastProgress = performance.now();
      self.postMessage({ type: "progress", data: search.progress() });
    }
    setTimeout(step, 0);
  } catch (error) {
    self.postMessage({ type: "error", data: String(error) });
    search?.free();
    search = undefined;
  }
}

self.onmessage = async ({ data }) => {
  if (data.type === "stop") {
    stopped = true;
    return;
  }
  if (data.type !== "start") return;
  try {
    await init();
    search = new WorkerSearch(data.data);
    self.postMessage({ type: "progress", data: search.progress() });
    setTimeout(step, 0);
  } catch (error) {
    self.postMessage({ type: "error", data: String(error) });
  }
};
