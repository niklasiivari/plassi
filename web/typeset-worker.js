// Keep the cached typesetting engine and fonts in a worker, loaded on first print.
import init, { typeset } from "./typeset/plassi_typeset.js";

const ready = init();
let queue = Promise.resolve();
self.onmessage = ({ data: { id, data } }) => {
  queue = queue.then(async () => {
    try {
      await ready;
      const pdf = typeset(data);
      self.postMessage({ id, pdf }, [pdf.buffer]);
    } catch (error) {
      self.postMessage({ id, error: String(error) });
    }
  });
};
