#!/usr/bin/env node
// The susbot command: runs the native binary from the platform package npm
// installed next to this one (@sitefig/susbot-<platform>-<arch>, an optional
// dependency). SUSBOT_BINARY points at another binary instead.
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const exe = process.platform === 'win32' ? 'susbot.exe' : 'susbot';
const pkg = `@sitefig/susbot-${process.platform}-${process.arch}`;

function binary() {
  if (process.env.SUSBOT_BINARY) return process.env.SUSBOT_BINARY;
  try {
    return require.resolve(`${pkg}/bin/${exe}`);
  } catch {
    console.error(
      `susbot: no prebuilt binary for ${process.platform}-${process.arch} (the optional package ${pkg} is not installed).\n` +
        'Reinstall without --no-optional / --omit=optional, or install the CLI with `cargo install susbot` or `pip install susbot`.\n' +
        'The JavaScript API (import { Analysis } from "susbot") works on every platform.',
    );
    process.exit(2);
  }
}

const result = spawnSync(binary(), process.argv.slice(2), { stdio: 'inherit', windowsHide: true });
if (result.error) {
  console.error(`susbot: ${result.error.message}`);
  process.exit(2);
}
if (result.signal) process.kill(process.pid, result.signal);
process.exit(result.status ?? 2);
