// The UI and workers communicate only through messages, without shared memory
// or cross-origin isolation requirements. Each solver job owns its worker.
import { plassiWake } from "./plassi.js";
export function startSearch(data) {
  const worker = new Worker(new URL("./solver-worker.js", import.meta.url), { type: "module" });
  let result;
  let progress;
  let finished = false;
  worker.onmessage = ({ data }) => {
    if (finished) return;
    if (data.type === "progress") progress = data.data;
    else {
      result = data;
      finished = true;
      worker.terminate();
    }
    plassiWake();
  };
  worker.onerror = (event) => {
    event.preventDefault();
    if (finished) return;
    result = { type: "error", data: event.message || "Solver worker unavailable" };
    finished = true;
    worker.terminate();
    plassiWake();
  };
  worker.onmessageerror = () => {
    if (finished) return;
    result = { type: "error", data: "Could not read solver worker result" };
    finished = true;
    worker.terminate();
    plassiWake();
  };
  worker.postMessage({ type: "start", data });
  return {
    poll() { const value = result; result = undefined; return value; },
    progress() { return progress; },
    stop() { if (!finished) worker.postMessage({ type: "stop" }); },
    cancel() { finished = true; worker.terminate(); },
  };
}

let printer;
let fallbackTypesetter;
let sequence = 0;
const pending = new Map();

function unavailable(message) {
  const error = new Error(message);
  error.workerUnavailable = true;
  return error;
}

function typesetterWorker() {
  if (printer) return printer;
  const worker = new Worker(new URL("./typeset-worker.js", import.meta.url), { type: "module" });
  worker.onmessage = ({ data }) => {
    const request = pending.get(data.id);
    if (!request) return;
    pending.delete(data.id);
    if (data.error !== undefined) request.reject(new Error(data.error));
    else request.resolve(data.pdf);
    plassiWake();
  };
  const failed = (message) => {
    if (printer !== worker) return;
    for (const request of pending.values()) request.reject(unavailable(message));
    pending.clear();
    worker.terminate();
    if (printer === worker) printer = undefined;
    plassiWake();
  };
  worker.onerror = (event) => {
    event.preventDefault();
    failed(event.message || "PDF worker unavailable");
  };
  worker.onmessageerror = () => failed("Could not read PDF worker result");
  printer = worker;
  return worker;
}

export async function typeset(data) {
  try {
    let worker;
    try { worker = typesetterWorker(); }
    catch (error) { throw unavailable(String(error)); }
    return await new Promise((resolve, reject) => {
      const id = ++sequence;
      pending.set(id, { resolve, reject });
      try { worker.postMessage({ id, data }); }
      catch (error) { pending.delete(id); reject(unavailable(String(error))); }
    });
  } catch (error) {
    // Retain printing on browsers or deployments where module workers fail.
    // Normal document errors remain errors and do not run twice on the UI.
    if (!error.workerUnavailable) throw error;
    fallbackTypesetter ??= import("./typeset/plassi_typeset.js").then(async (module) => {
      await module.default();
      return module;
    }).catch((error) => { fallbackTypesetter = undefined; throw error; });
    return (await fallbackTypesetter).typeset(data);
  }
}

let crcTable;
function crc32(bytes) {
  crcTable ??= Uint32Array.from({ length: 256 }, (_, index) => {
    let value = index;
    for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ ((value & 1) ? 0xedb88320 : 0);
    return value >>> 0;
  });
  let crc = 0xffffffff;
  for (let index = 0; index < bytes.length; index++) crc = (crc >>> 8) ^ crcTable[(crc ^ bytes[index]) & 255];
  return (crc ^ 0xffffffff) >>> 0;
}

// Store PDF streams without recompressing them, matching the Rust ZIP writer.
// Keeping the transferred buffers here avoids copying every PDF into app Wasm
// and then copying the finished archive back into JavaScript.
export function pdfArchive(files) {
  if (files.length > 65535) throw new Error("Too many PDFs for an archive");
  const encoder = new TextEncoder();
  const entries = files.map(({ name, bytes }) => ({ name: encoder.encode(name), bytes }));
  let size = 22;
  for (const entry of entries) {
    if (entry.name.length > 65535) throw new Error("PDF filename is too long");
    size += 76 + 2 * entry.name.length + entry.bytes.length;
  }
  if (size > 0xffffffff) throw new Error("PDF archive is too large");
  const bytes = new Uint8Array(size);
  const view = new DataView(bytes.buffer);
  const u16 = (offset, value) => view.setUint16(offset, value, true);
  const u32 = (offset, value) => view.setUint32(offset, value, true);
  let offset = 0;
  for (const entry of entries) {
    entry.offset = offset;
    entry.crc = crc32(entry.bytes);
    u32(offset, 0x04034b50);
    u16(offset + 4, 20);
    u16(offset + 6, 0x0800); // UTF-8 filenames.
    u16(offset + 12, 0x0021); // 1980-01-01.
    u32(offset + 14, entry.crc);
    u32(offset + 18, entry.bytes.length);
    u32(offset + 22, entry.bytes.length);
    u16(offset + 26, entry.name.length);
    bytes.set(entry.name, offset + 30);
    bytes.set(entry.bytes, offset + 30 + entry.name.length);
    offset += 30 + entry.name.length + entry.bytes.length;
  }
  const directory = offset;
  for (const entry of entries) {
    u32(offset, 0x02014b50);
    u16(offset + 4, 20);
    u16(offset + 6, 20);
    u16(offset + 8, 0x0800);
    u16(offset + 14, 0x0021);
    u32(offset + 16, entry.crc);
    u32(offset + 20, entry.bytes.length);
    u32(offset + 24, entry.bytes.length);
    u16(offset + 28, entry.name.length);
    u32(offset + 42, entry.offset);
    bytes.set(entry.name, offset + 46);
    offset += 46 + entry.name.length;
  }
  u32(offset, 0x06054b50);
  u16(offset + 8, entries.length);
  u16(offset + 10, entries.length);
  u32(offset + 12, offset - directory);
  u32(offset + 16, directory);
  return bytes;
}
