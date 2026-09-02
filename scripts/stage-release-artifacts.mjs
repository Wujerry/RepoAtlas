#!/usr/bin/env node

import { copyFileSync, mkdirSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(fileURLToPath(new URL('../', import.meta.url)));

function usage() {
  console.error(
    'Usage: node scripts/stage-release-artifacts.mjs --target TARGET --output DIR [--root DIR] [--label LABEL]',
  );
}

function parseArgs(argv) {
  const values = {
    // Cargo resolves the workspace target dir at the repository root even
    // when tauri-action invokes cargo from src-tauri.
    root: path.join(repoRoot, 'target'),
    label: undefined,
    target: undefined,
    output: undefined,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--help' || argument === '-h') {
      usage();
      process.exit(0);
    }
    if (!['--label', '--root', '--target', '--output'].includes(argument)) {
      throw new Error(`unknown option ${argument}`);
    }
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) {
      throw new Error(`${argument} requires a value`);
    }
    values[argument.slice(2)] = value;
    index += 1;
  }

  if (!values.target) throw new Error('--target is required');
  if (!values.output) throw new Error('--output is required');
  return values;
}

function walk(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walk(entryPath));
    else if (entry.isFile()) files.push(entryPath);
  }
  return files;
}

function isReleaseAsset(filePath) {
  return /(?:\.msi|\.exe|\.dmg|\.app\.tar\.gz|\.zip|\.sig)$/i.test(filePath);
}

// Inserts the label before the installer extension so `foo.exe` becomes
// `foo-UNSIGNED-BETA.exe` and its sidecar signature `foo.exe.sig` becomes
// `foo-UNSIGNED-BETA.exe.sig`, preserving the updater `.sig` pairing.
function labeledFileName(fileName, label) {
  if (!label) return fileName;
  const isSignature = fileName.toLowerCase().endsWith('.sig');
  const base = isSignature ? fileName.slice(0, -4) : fileName;
  const extension = path.extname(base);
  if (!extension) return `${base}-${label}${isSignature ? '.sig' : ''}`;
  const stem = base.slice(0, base.length - extension.length);
  return `${stem}-${label}${extension}${isSignature ? '.sig' : ''}`;
}

try {
  const { label, root, target, output } = parseArgs(process.argv.slice(2));
  const bundleRoot = path.resolve(root, target, 'release', 'bundle');
  if (!statSync(bundleRoot, { throwIfNoEntry: false })?.isDirectory()) {
    throw new Error(`bundle directory does not exist: ${bundleRoot}`);
  }

  const assets = walk(bundleRoot).filter(isReleaseAsset);
  if (assets.length === 0) throw new Error(`no release assets found below ${bundleRoot}`);

  const outputRoot = path.resolve(output);
  mkdirSync(outputRoot, { recursive: true });
  for (const asset of assets) {
    const stagedName = `${target}-${labeledFileName(path.basename(asset), label)}`;
    const destination = path.join(outputRoot, stagedName);
    copyFileSync(asset, destination);
    console.log(`${path.relative(repoRoot, asset)} -> ${path.relative(repoRoot, destination)}`);
  }
} catch (error) {
  usage();
  console.error(`Release artifact staging failed: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
