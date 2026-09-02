#!/usr/bin/env node

import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(fileURLToPath(new URL('../', import.meta.url)));

function parseArgs(argv) {
  const values = {
    artifacts: undefined,
    baseUrl: undefined,
    label: undefined,
    output: undefined,
    version: undefined,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--help' || argument === '-h') {
      console.log(
        'Usage: node scripts/build-updater-manifest.mjs --version X.Y.Z --artifacts DIR --output FILE --base-url URL',
      );
      process.exit(0);
    }
    if (!['--artifacts', '--base-url', '--label', '--output', '--version'].includes(argument)) {
      throw new Error(`unknown option ${argument}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${argument} requires a value`);
    const name = argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    values[name] = value;
    index += 1;
  }
  for (const name of ['artifacts', 'baseUrl', 'output', 'version']) {
    if (!values[name]) throw new Error(`--${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
  }
  if (!/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.test(values.version)) {
    throw new Error(`version must be X.Y.Z or X.Y.Z-<prerelease>, received ${JSON.stringify(values.version)}`);
  }
  return values;
}

function listFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const filePath = path.join(directory, entry.name);
    if (entry.isDirectory()) return listFiles(filePath);
    return entry.isFile() ? [filePath] : [];
  });
}

function encodedUrl(baseUrl, fileName) {
  return `${baseUrl.replace(/\/$/, '')}/${encodeURIComponent(fileName)}`;
}

function targetFiles(files, target) {
  const prefix = `${target}-`;
  return files.filter((file) => path.basename(file).startsWith(prefix));
}

function chooseUpdateBundle(files, target, label) {
  const names = files
    .map((file) => path.basename(file))
    .filter((name) => !name.endsWith('.sig'));
  // Tauri v2 with createUpdaterArtifacts: true signs the raw installers
  // (.exe/.msi); v1Compatible mode produces .nsis.zip/.msi.zip instead.
  const preferred = target === 'x86_64-pc-windows-msvc'
    ? ['-setup.exe', '.msi', '.nsis.zip', '.msi.zip']
    : ['.app.tar.gz'];
  for (const suffix of preferred) {
    const match = names.find((name) => plainReleaseName(name, label).endsWith(suffix));
    if (match) return match;
  }
  throw new Error(`no updater bundle found for ${target}; expected ${preferred.join(' or ')}`);
}

// Release labels such as UNSIGNED-BETA are inserted before the installer
// extension; strip them so suffix matching still finds the bundle.
function plainReleaseName(name, label) {
  return label ? name.split(`-${label}`).join('') : name;
}

try {
  const { artifacts, baseUrl, label, output, version } = parseArgs(process.argv.slice(2));
  const artifactRoot = path.resolve(artifacts);
  if (!statSync(artifactRoot, { throwIfNoEntry: false })?.isDirectory()) {
    throw new Error(`artifact directory does not exist: ${artifactRoot}`);
  }
  const files = listFiles(artifactRoot);
  const targetMap = {
    'windows-x86_64': 'x86_64-pc-windows-msvc',
    'darwin-x86_64': 'x86_64-apple-darwin',
    'darwin-aarch64': 'aarch64-apple-darwin',
  };
  const platforms = {};
  for (const [platform, target] of Object.entries(targetMap)) {
    const candidates = targetFiles(files, target);
    const updateName = chooseUpdateBundle(candidates, target, label);
    const updatePath = candidates.find((file) => path.basename(file) === updateName);
    const signaturePath = candidates.find((file) => path.basename(file) === `${updateName}.sig`);
    if (!updatePath || !signaturePath) {
      throw new Error(`missing updater signature for ${target}: ${updateName}.sig`);
    }
    platforms[platform] = {
      signature: readFileSync(signaturePath, 'utf8').trim(),
      url: encodedUrl(baseUrl, updateName),
    };
  }

  const manifest = {
    version,
    notes: `RepoAtlas ${version} draft release. See CHANGELOG.md for release notes.`,
    pub_date: new Date().toISOString(),
    platforms,
  };
  const outputPath = path.resolve(output);
  writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
  console.log(`Updater manifest written to ${path.relative(repoRoot, outputPath)}`);
} catch (error) {
  console.error(`Updater manifest generation failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
