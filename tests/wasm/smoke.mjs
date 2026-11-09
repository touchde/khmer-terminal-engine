// Loads the KTE WebAssembly build and checks it from JavaScript, the way the
// xterm.js integration will use it.
//
//   cargo build -p kte-capi --release --target wasm32-unknown-unknown
//   node tests/wasm/smoke.mjs [path/to/kte_capi.wasm]

import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const path = process.argv[2] ?? 'target/wasm32-unknown-unknown/release/kte_capi.wasm';
const { instance } = await WebAssembly.instantiate(readFileSync(path), {});
const kte = instance.exports;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Copies a JS string into wasm memory; returns [ptr, len, free]. */
function input(text) {
  const bytes = encoder.encode(text);
  const ptr = kte.kte_alloc(bytes.length);
  new Uint8Array(kte.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length, () => kte.kte_free(ptr, bytes.length)];
}

function strWidth(text, model = 0) {
  const [ptr, len, free] = input(text);
  try {
    return kte.kte_str_width(ptr, len, model) >>> 0;
  } finally {
    free();
  }
}

assert.equal(strWidth('សួស្តី ពិភពលោក!'), 21);
assert.equal(strWidth('សួស្តី ពិភពលោក!', 1), 13);
assert.equal(strWidth('ok'), 2);

// Terminal: feed text, read the row back.
const term = kte.kte_terminal_new(4, 40, 0);
const text = 'ស្ត្រី ok';
const [ptr, len, free] = input(text);
kte.kte_terminal_feed(term, ptr, len);
free();

const out = kte.kte_alloc(256);
const n = kte.kte_terminal_row_text(term, 0, out, 256) >>> 0;
assert.equal(decoder.decode(new Uint8Array(kte.memory.buffer, out, n)), text);
kte.kte_free(out, 256);
kte.kte_terminal_free(term);

console.log('wasm ok:', path);
