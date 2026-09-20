#!/usr/bin/env node
// Assembles the npm packages for a release into an output folder:
//
//   node scripts/npm-pack.mjs <binaries-dir> <out-dir>
//
// <binaries-dir> holds one CLI build per platform, named susbot-<platform>-<arch>
// (".exe" on Windows), as the release workflow uploads them. <out-dir> gets
// susbot/ (the main package: npm/susbot plus its generated wasm/) and one
// susbot-<platform>-<arch>/ per binary, published as @sitefig/susbot-<platform>-<arch>. Every package gets the version of the
// root Cargo.toml; it fails when npm/susbot/package.json disagrees or a
// platform listed in optionalDependencies has no binary.
import { readFileSync, writeFileSync, mkdirSync, cpSync, rmSync, existsSync, chmodSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const [binDir, outDir] = process.argv.slice(2);
if (!binDir || !outDir) {
  console.error('usage: node scripts/npm-pack.mjs <binaries-dir> <out-dir>');
  process.exit(2);
}
const root = new URL('../', import.meta.url);
const version = readFileSync(new URL('Cargo.toml', root), 'utf8').match(/^version = "([^"]+)"/m)[1];
const mainDir = new URL('npm/susbot/', root);
const main = JSON.parse(readFileSync(new URL('package.json', mainDir), 'utf8'));
const fail = (msg) => {
  console.error(`npm-pack: ${msg}`);
  process.exit(1);
};
if (main.version !== version) fail(`npm/susbot/package.json is ${main.version}, Cargo.toml is ${version}`);
if (!existsSync(new URL('wasm/susbot_wasm_bg.wasm', mainDir))) fail('npm/susbot/wasm/ is missing: run scripts/build-npm-wasm.sh');

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const available = new Set(readdirSync(binDir));
for (const [name, want] of Object.entries(main.optionalDependencies)) {
  if (want !== version) fail(`optionalDependencies.${name} is ${want}, expected ${version}`);
  const [, platform, arch] = name.match(/^@sitefig\/susbot-([a-z0-9]+)-([a-z0-9]+)$/);
  const exe = platform === 'win32' ? '.exe' : '';
  const binary = `susbot-${platform}-${arch}${exe}`;
  if (!available.has(binary)) fail(`no binary ${binary} in ${binDir}`);
  // The folder keeps the unscoped spelling; package.json carries the scope.
  const dir = join(outDir, `susbot-${platform}-${arch}`);
  mkdirSync(join(dir, 'bin'), { recursive: true });
  cpSync(join(binDir, binary), join(dir, 'bin', `susbot${exe}`));
  chmodSync(join(dir, 'bin', `susbot${exe}`), 0o755);
  cpSync(new URL('LICENSE.md', mainDir), join(dir, 'LICENSE.md'));
  writeFileSync(
    join(dir, 'package.json'),
    JSON.stringify(
      {
        name,
        version,
        description: `The susbot command for ${platform}-${arch}. Install the susbot package instead; it picks this one.`,
        homepage: main.homepage,
        repository: main.repository,
        license: main.license,
        author: main.author,
        publishConfig: { access: 'public' },
        os: [platform],
        cpu: [arch],
        files: ['bin/', 'LICENSE.md'],
        preferUnplugged: true,
      },
      null,
      2,
    ) + '\n',
  );
  writeFileSync(join(dir, 'README.md'), `# ${name}\n\nThe prebuilt \`susbot\` binary for ${platform}-${arch}. Install [@sitefig/susbot](https://www.npmjs.com/package/@sitefig/susbot) instead; npm picks this package for your platform.\n`);
  console.log(`${name}@${version}`);
}

cpSync(mainDir, join(outDir, 'susbot'), { recursive: true, filter: (src) => !/[\\/](test|node_modules)([\\/]|$)/.test(src) });
console.log(`susbot@${version}`);
