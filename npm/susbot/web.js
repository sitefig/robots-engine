// Browser and bundler entry point: await init() once before anything else.
// Without an argument the .wasm file is fetched next to this module, which
// bundlers such as Vite and webpack resolve and copy.
import initWasm from './wasm/susbot_wasm.js';

let ready;

export function init(input) {
  ready ??= initWasm(input === undefined ? undefined : { module_or_path: input });
  return ready.then(() => undefined);
}

export * from './lib/api.js';
