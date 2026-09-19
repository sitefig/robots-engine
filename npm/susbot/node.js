// Node entry point: the engine is loaded synchronously from the package, so
// everything is ready on import. init() exists for code shared with browsers.
import { readFileSync } from 'node:fs';
import { initSync } from './wasm/susbot_wasm.js';

initSync({ module: readFileSync(new URL('./wasm/susbot_wasm_bg.wasm', import.meta.url)) });

export async function init() {}

export * from './lib/api.js';
