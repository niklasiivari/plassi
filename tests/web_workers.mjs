// Run with: node --test tests/web_workers.mjs
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

async function adapter(t, Worker) {
  const directory = await mkdtemp(path.join(tmpdir(), 'plassi-workers-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const previous = globalThis.Worker;
  globalThis.Worker = Worker;
  t.after(() => { globalThis.Worker = previous; });
  await writeFile(path.join(directory, 'package.json'), '{"type":"module"}');
  await writeFile(path.join(directory, 'workers.js'), await readFile(new URL('../web/workers.js', import.meta.url)));
  // The generated Wasm glue is replaced at its existing module boundary. The
  // worker adapter itself is imported without rewriting its implementation.
  await writeFile(path.join(directory, 'plassi.js'), 'export function plassiWake() {}');
  await mkdir(path.join(directory, 'typeset'));
  await writeFile(path.join(directory, 'typeset/plassi_typeset.js'), `
    export default async function init() {}
    export function typeset(data) { return new TextEncoder().encode(data); }
  `);
  return import(pathToFileURL(path.join(directory, 'workers.js')));
}

class WorkerStub {
  static instances = [];
  constructor(url, options) {
    this.url = url;
    this.options = options;
    this.messages = [];
    this.terminated = false;
    WorkerStub.instances.push(this);
  }
  postMessage(message) { this.messages.push(message); }
  terminate() { this.terminated = true; }
}

function lastWorker() { return WorkerStub.instances.at(-1); }

test('solver Stop retains the worker until its best outcome, preserving i64 JSON', async t => {
  const { startSearch } = await adapter(t, WorkerStub);
  const input = '{"seed":9223372036854775806}';
  const job = startSearch(input);
  const worker = lastWorker();
  assert.equal(worker.options.type, 'module');
  assert.deepEqual(worker.messages, [{ type: 'start', data: input }]);
  const progress = '[9223372036854775806,123]';
  worker.onmessage({ data: { type: 'progress', data: progress } });
  assert.equal(job.progress(), progress);
  job.stop();
  assert.deepEqual(worker.messages.at(-1), { type: 'stop' });
  assert.equal(worker.terminated, false);
  const outcome = '{"cost":9223372036854775806,"assignment":{"0":"T1:left:0"}}';
  worker.onmessage({ data: { type: 'done', data: outcome } });
  assert.equal(worker.terminated, true);
  assert.deepEqual(job.poll(), { type: 'done', data: outcome });
  assert.equal(job.poll(), undefined);
  const count = worker.messages.length;
  job.stop();
  assert.equal(worker.messages.length, count);
});

test('cancel terminates an obsolete solver and ignores queued messages', async t => {
  const { startSearch } = await adapter(t, WorkerStub);
  const job = startSearch('{}');
  const worker = lastWorker();
  job.cancel();
  assert.equal(worker.terminated, true);
  worker.onmessage({ data: { type: 'done', data: 'obsolete' } });
  assert.equal(job.poll(), undefined);
});

test('worker startup and message errors are delivered to the caller', async t => {
  const { startSearch } = await adapter(t, WorkerStub);
  const job = startSearch('{}');
  let prevented = false;
  lastWorker().onerror({ message: 'loading failed', preventDefault() { prevented = true; } });
  assert.equal(prevented, true);
  assert.deepEqual(job.poll(), { type: 'error', data: 'loading failed' });
  assert.equal(lastWorker().terminated, true);
  const second = startSearch('{}');
  lastWorker().onmessageerror();
  assert.equal(second.poll().type, 'error');
  assert.equal(lastWorker().terminated, true);
});

test('a PDF worker result retains the transferred buffer without an adapter copy', async t => {
  const { typeset } = await adapter(t, WorkerStub);
  const pdf = new Uint8Array([37, 80, 68, 70]);
  const pending = typeset('template');
  const worker = lastWorker();
  assert.equal(worker.messages[0].data, 'template');
  worker.onmessage({ data: { id: worker.messages[0].id, pdf } });
  assert.equal(await pending, pdf);
});

test('unavailable module workers retain the direct typesetter fallback', async t => {
  const { typeset } = await adapter(t, class {
    constructor() { throw new Error('module workers unavailable'); }
  });
  assert.deepEqual(await typeset('template'), new TextEncoder().encode('template'));
});

test('a worker load failure falls back, while a document error stays an error', async t => {
  const { typeset } = await adapter(t, WorkerStub);
  const first = typeset('template');
  lastWorker().onerror({ message: 'module failed to load', preventDefault() {} });
  assert.deepEqual(await first, new TextEncoder().encode('template'));
  const second = typeset('invalid template');
  const worker = lastWorker();
  worker.onmessage({ data: { id: worker.messages[0].id, error: 'invalid template' } });
  await assert.rejects(second, /invalid template/);
});

test('stored PDF ZIP keeps UTF-8 names, bytes, CRCs and directory offsets', async t => {
  const { pdfArchive } = await adapter(t, WorkerStub);
  const files = [
    { name: '<project> - seating.pdf', bytes: new TextEncoder().encode('123456789') },
    { name: '<project> - istumajärjestys.pdf', bytes: new Uint8Array() },
  ];
  const archive = pdfArchive(files);
  const view = new DataView(archive.buffer);
  const decoder = new TextDecoder();
  let offset = 0;
  const positions = [];
  for (const [index, file] of files.entries()) {
    positions.push(offset);
    assert.equal(view.getUint32(offset, true), 0x04034b50);
    assert.equal(view.getUint16(offset + 6, true), 0x0800);
    assert.equal(view.getUint16(offset + 8, true), 0);
    assert.equal(view.getUint32(offset + 14, true), index === 0 ? 0xcbf43926 : 0);
    const size = view.getUint32(offset + 18, true);
    assert.equal(size, file.bytes.length);
    const length = view.getUint16(offset + 26, true);
    assert.equal(decoder.decode(archive.subarray(offset + 30, offset + 30 + length)), file.name);
    assert.deepEqual(archive.subarray(offset + 30 + length, offset + 30 + length + size), file.bytes);
    offset += 30 + length + size;
  }
  const directory = offset;
  for (const [index, file] of files.entries()) {
    assert.equal(view.getUint32(offset, true), 0x02014b50);
    assert.equal(view.getUint32(offset + 42, true), positions[index]);
    const length = view.getUint16(offset + 28, true);
    assert.equal(decoder.decode(archive.subarray(offset + 46, offset + 46 + length)), file.name);
    offset += 46 + length;
  }
  assert.equal(view.getUint32(offset, true), 0x06054b50);
  assert.equal(view.getUint16(offset + 10, true), files.length);
  assert.equal(view.getUint32(offset + 12, true), offset - directory);
  assert.equal(view.getUint32(offset + 16, true), directory);
  assert.equal(offset + 22, archive.length);
  assert.equal(pdfArchive([]).length, 22);
});
