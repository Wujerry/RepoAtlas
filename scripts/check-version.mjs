#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repoRoot = path.resolve(fileURLToPath(new URL('../', import.meta.url)));
const packagePath = path.join(repoRoot, 'package.json');
const cargoPath = path.join(repoRoot, 'Cargo.toml');
const tauriPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');

function fail(message) {
  console.error(`Version check failed: ${message}`);
  process.exitCode = 1;
}

function readText(filePath) {
  try {
    return readFileSync(filePath, 'utf8');
  } catch (error) {
    throw new Error(`cannot read ${path.relative(repoRoot, filePath)}: ${error.message}`);
  }
}

function readJson(filePath) {
  try {
    return JSON.parse(readText(filePath));
  } catch (error) {
    throw new Error(`cannot parse ${path.relative(repoRoot, filePath)}: ${error.message}`);
  }
}

function parseArgs(argv) {
  let requestedTag;

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];

    if (argument === '--help' || argument === '-h') {
      console.log('Usage: node scripts/check-version.mjs [--tag vX.Y.Z or vX.Y.Z-<prerelease>]');
      process.exit(0);
    }

    if (argument === '--tag') {
      requestedTag = argv[index + 1];
      index += 1;
      if (!requestedTag) {
        throw new Error('--tag requires a value such as v0.1.0-beta.2');
      }
      continue;
    }

    if (argument.startsWith('--')) {
      throw new Error(`unknown option ${argument}`);
    }

    if (requestedTag) {
      throw new Error(`unexpected argument ${argument}`);
    }

    requestedTag = argument;
  }

  if (!requestedTag && process.env.GITHUB_REF_TYPE === 'tag') {
    requestedTag = process.env.GITHUB_REF_NAME;
  }

  return requestedTag;
}

function readCargoWorkspaceVersion() {
  const cargoText = readText(cargoPath);
  const workspacePackage = cargoText.match(
    /\[workspace\.package\]([\s\S]*?)(?=\r?\n\[|$)/,
  )?.[1];

  if (!workspacePackage) {
    throw new Error('Cargo.toml is missing [workspace.package]');
  }

  const version = workspacePackage.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) {
    throw new Error('Cargo.toml [workspace.package] is missing version');
  }

  return version;
}

const VERSION_PATTERN =
  /^(0|[1-9][0-9]*)[.](0|[1-9][0-9]*)[.](0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:[.][0-9A-Za-z-]+)*))?$/;

function assertVersion(version, source) {
  if (!VERSION_PATTERN.test(version)) {
    throw new Error(
      `${source} has unsupported version ${JSON.stringify(version)}; expected X.Y.Z or X.Y.Z-<prerelease>`,
    );
  }
}

function splitTag(tag) {
  const versionBody = VERSION_PATTERN.source.slice(1, -1);
  const match = new RegExp(`^v${versionBody}$`).exec(tag);
  if (!match) {
    throw new Error(`tag ${JSON.stringify(tag)} must be exactly v plus the project version`);
  }
  return { version: tag.slice(1), prerelease: match[4] ?? null };
}

try {
  const requestedTag = parseArgs(process.argv.slice(2));
  const packageVersion = readJson(packagePath).version;
  const cargoVersion = readCargoWorkspaceVersion();
  const tauriVersion = readJson(tauriPath).version;

  const versions = new Map([
    ['package.json', packageVersion],
    ['Cargo.toml [workspace.package]', cargoVersion],
    ['src-tauri/tauri.conf.json', tauriVersion],
  ]);

  for (const [source, version] of versions) {
    if (typeof version !== 'string') {
      throw new Error(`${source} is missing a string version`);
    }
    assertVersion(version, source);
  }

  const uniqueVersions = new Set(versions.values());
  if (uniqueVersions.size !== 1) {
    throw new Error(
      [...versions.entries()].map(([source, version]) => `${source}=${version}`).join(', '),
    );
  }

  const [version] = uniqueVersions;
  if (requestedTag) {
    const parsed = splitTag(requestedTag);
    if (parsed.version !== version) {
      throw new Error(
        `tag ${requestedTag} does not match project version ${version}; the release tag must be exactly v${version}, prerelease suffix included`,
      );
    }
  }
  const prerelease = VERSION_PATTERN.exec(version)?.[4] ?? null;
  if (prerelease) {
    console.log(`RepoAtlas version check passed: ${version} (prerelease -${prerelease})`);
  } else {
    console.log(`RepoAtlas version check passed: ${version}${requestedTag ? ` (${requestedTag})` : ''}`);
  }
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
