#!/usr/bin/env node
// Version planning and crates.io publishing for the GitHub workflows.

import { spawnSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
export const CRATES = [
  'tilt-ui-core',
  'tilt-ui-html',
  'tilt-ui-css',
  'tilt-ui-icons',
  'tilt-ui-macros',
  'tilt-ui-build',
  'tilt-ui-runtime',
  'tilt-ui',
];

const SEMVER = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/;
const RELEASE_BRANCH = /^release-((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))$/;
const DEPENDENCY = /^(tilt-ui(?:-[a-z]+)? = \{ path = "crates\/[^\"]+")(?:, version = "[^"]+")?( \})$/gm;
const USER_AGENT = 'tilt-ui-release/1 (https://github.com/exepta/tilt-ui)';

export function workspaceVersion(root = ROOT) {
  const manifest = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  const section = manifest.split('[workspace.package]')[1]?.split(/\n\[/)[0];
  const version = section?.match(/^version = "([^"]+)"$/m)?.[1];
  if (!version) throw new Error('Workspace version is missing from Cargo.toml');
  return version;
}

export function baseVersion(root = ROOT) {
  const version = workspaceVersion(root);
  if (!SEMVER.test(version)) {
    throw new Error(`Workspace version must be a stable x.y.z: ${version}`);
  }
  return version;
}

async function requestJson(url, token = '', missing = null) {
  const headers = {
    Accept: 'application/vnd.github+json',
    'User-Agent': USER_AGENT,
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  const response = await fetch(url, {
    headers,
    signal: AbortSignal.timeout(30_000),
  });
  if (response.status === 404) return missing;
  if (!response.ok) throw new Error(`HTTP ${response.status} while checking ${url}`);
  return response.json();
}

function githubUrl(repo, path) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo)) {
    throw new Error('GITHUB_REPOSITORY is missing or invalid');
  }
  return `https://api.github.com/repos/${repo}/${path}`;
}

async function tagExists(repo, tag, token) {
  const url = githubUrl(repo, `git/ref/tags/${encodeURIComponent(tag)}`);
  return (await requestJson(url, token)) !== null;
}

async function matchingTags(repo, prefix, token) {
  const url = githubUrl(repo, `git/matching-refs/tags/${encodeURIComponent(prefix)}`);
  const refs = (await requestJson(url, token, [])) ?? [];
  return refs.map((item) => item.ref.replace(/^refs\/tags\//, ''));
}

async function crateVersions(name) {
  const versions = new Set();
  for (let page = 1; ; page += 1) {
    const url = `https://crates.io/api/v1/crates/${name}/versions?page=${page}&per_page=100`;
    const data = await requestJson(url);
    if (data === null) return versions;
    const batch = data.versions ?? [];
    for (const item of batch) versions.add(item.num);
    if (batch.length < 100) return versions;
  }
}

async function publishedVersions() {
  const values = await Promise.all(CRATES.map((name) => crateVersions(name)));
  return Object.fromEntries(CRATES.map((name, index) => [name, values[index]]));
}

export function stableCollisions(version, published, hasTag) {
  const hits = CRATES.filter((name) => published[name].has(version))
    .map((name) => `crates.io/${name}`);
  if (hasTag) hits.push(`GitHub tag v${version}`);
  return hits;
}

export function nextRc(base, published, tags) {
  const prefix = `${base}-rc.`;
  const used = new Set(CRATES.flatMap((name) => [...published[name]]));
  for (const tag of tags) used.add(tag.replace(/^v/, ''));
  const numbers = [...used]
    .filter((version) => version.startsWith(prefix) && /^\d+$/.test(version.slice(prefix.length)))
    .map((version) => Number(version.slice(prefix.length)));
  return `${prefix}${Math.max(0, ...numbers) + 1}`;
}

export function findMergedRcPr(pulls) {
  return pulls.find((pr) => pr.merged_at
    && pr.base?.ref === 'main'
    && pr.title?.startsWith('RC:')
    && !pr.head?.ref?.startsWith('release-'))?.number ?? null;
}

async function mergedRcPr(repo, sha, token) {
  if (!/^[0-9a-f]{40}$/.test(sha ?? '')) {
    throw new Error('GITHUB_SHA is missing or invalid');
  }
  const pulls = (await requestJson(githubUrl(repo, `commits/${sha}/pulls`), token, [])) ?? [];
  return findMergedRcPr(pulls);
}

function writeOutput(values) {
  const lines = Object.entries(values).map(([key, value]) => `${key}=${value}`).join('\n') + '\n';
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, lines);
  else process.stdout.write(lines);
}

function summary(message) {
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${message}\n`);
  }
  console.log(message);
}

export async function prCheck() {
  const event = JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
  const pr = event.pull_request;
  const branch = pr.head.ref;
  const title = pr.title;
  const stable = branch.match(RELEASE_BRANCH);
  const rc = title.startsWith('RC:');
  if (!stable && !rc) {
    summary('Regular PR: CI tests run, no release is planned.');
    return;
  }
  const base = baseVersion();
  const repo = process.env.GITHUB_REPOSITORY;
  const token = process.env.GH_TOKEN ?? '';
  const published = await publishedVersions();
  const collisions = stableCollisions(base, published, await tagExists(repo, `v${base}`, token));
  if (stable) {
    const version = stable[1];
    if (rc) throw new Error('An RC: PR cannot use a release-x.y.z branch.');
    if (version !== base) {
      throw new Error(`Branch release-${version} differs from Cargo.toml version ${base}.`);
    }
    if (collisions.length) {
      throw new Error(`v${version} already exists: ${collisions.join(', ')}. Choose a new version.`);
    }
    summary(`Stable release planned from \`${branch}\`: \`v${version}\`. It publishes when the branch is pushed.`);
  } else {
    if (collisions.length) {
      throw new Error(`v${base} is already stable: ${collisions.join(', ')}. Bump Cargo.toml before another RC.`);
    }
    const tags = await matchingTags(repo, `v${base}-rc.`, token);
    const candidate = nextRc(base, published, tags);
    summary(`RC candidate: \`v${candidate}\`. The final RC number is allocated after this PR merges into \`main\`.`);
  }
}

export async function plan() {
  const branch = process.env.GITHUB_REF_NAME;
  const base = baseVersion();
  const repo = process.env.GITHUB_REPOSITORY;
  const token = process.env.GH_TOKEN ?? '';
  const stable = branch.match(RELEASE_BRANCH);
  let version;
  let kind;
  if (stable) {
    version = stable[1];
    if (version !== base) {
      throw new Error(`Branch release-${version} differs from Cargo.toml version ${base}.`);
    }
    kind = 'stable';
  } else if (branch === 'main') {
    const pr = await mergedRcPr(repo, process.env.GITHUB_SHA, token);
    if (pr === null) {
      writeOutput({ publish: 'false' });
      summary('main stays a development branch; this push does not publish a release.');
      return;
    }
    kind = 'rc';
  } else {
    writeOutput({ publish: 'false' });
    return;
  }

  const published = await publishedVersions();
  const collisions = stableCollisions(base, published, await tagExists(repo, `v${base}`, token));
  if (collisions.length) {
    const detail = kind === 'stable'
      ? `v${base} already exists: ${collisions.join(', ')}.`
      : `v${base} is already stable; refusing to publish another RC.`;
    throw new Error(detail);
  }
  if (kind === 'rc') {
    const tags = await matchingTags(repo, `v${base}-rc.`, token);
    version = nextRc(base, published, tags);
  }
  const tag = `v${version}`;
  if (await tagExists(repo, tag, token)
    || CRATES.some((name) => published[name].has(version))) {
    throw new Error(`${tag} appeared while planning; refusing a duplicate release.`);
  }
  writeOutput({ publish: 'true', version, tag, kind });
  summary(`Planned ${kind} release \`${tag}\` for ${CRATES.join(', ')}.`);
}

export function prepare(version, root = ROOT) {
  if (!/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-rc\.[1-9]\d*)?$/.test(version)) {
    throw new Error(`Invalid release version: ${version}`);
  }
  const path = join(root, 'Cargo.toml');
  let manifest = readFileSync(path, 'utf8');
  const start = manifest.indexOf('[workspace.package]');
  const end = manifest.indexOf('\n[', start + 1);
  if (start < 0 || end < 0) throw new Error('Could not find workspace.package section');
  let section = manifest.slice(start, end);
  if (!/^version = "[^"]+"$/m.test(section)) {
    throw new Error('Could not update workspace.package.version');
  }
  section = section.replace(/^version = "[^"]+"$/m, `version = "${version}"`);
  manifest = manifest.slice(0, start) + section + manifest.slice(end);
  let count = 0;
  manifest = manifest.replace(DEPENDENCY, (_match, head, tail) => {
    count += 1;
    return `${head}, version = "=${version}"${tail}`;
  });
  if (count !== CRATES.length) {
    throw new Error(`Expected ${CRATES.length} internal dependencies, found ${count}`);
  }
  writeFileSync(path, manifest);
  summary(`Prepared workspace manifests for \`${version}\`. Cargo tests will refresh Cargo.lock.`);
}

function cargo(args) {
  const result = spawnSync('cargo', args, { cwd: ROOT, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`cargo ${args.join(' ')} failed with status ${result.status}`);
}

export async function publish(version) {
  if (!process.env.CARGO_REGISTRY_TOKEN) {
    throw new Error('CARGO_REGISTRY_TOKEN is missing from the crates-io GitHub environment.');
  }
  const current = workspaceVersion();
  if (current !== version) {
    throw new Error(`Manifest version ${current} does not match requested ${version}.`);
  }
  const published = await publishedVersions();
  const conflicts = CRATES.filter((name) => published[name].has(version));
  if (conflicts.length) {
    throw new Error(`Already on crates.io: ${conflicts.join(', ')} ${version}. Do not republish.`);
  }
  for (const name of CRATES) {
    summary(`Publishing \`${name} ${version}\` to crates.io`);
    // A dependent crate can be packaged after earlier crates reach the index.
    cargo(['package', '--package', name, '--locked', '--no-verify']);
    // The full workspace was tested before upload. Index propagation can make
    // isolated package verification fail between dependent crates.
    cargo(['publish', '--package', name, '--locked', '--no-verify']);
  }
}

async function main() {
  const [command, version, ...extra] = process.argv.slice(2);
  if (extra.length || !['pr-check', 'plan', 'prepare', 'publish'].includes(command)
    || (['prepare', 'publish'].includes(command) ? !version : version !== undefined)) {
    throw new Error('Usage: node scripts/release.mjs <pr-check|plan|prepare VERSION|publish VERSION>');
  }
  if (command === 'pr-check') await prCheck();
  if (command === 'plan') await plan();
  if (command === 'prepare') prepare(version);
  if (command === 'publish') await publish(version);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().catch((error) => {
    const message = error.message ?? String(error);
    console.error(`::warning title=Release version::${message}`);
    summary(`⚠️ Release blocked: ${message}`);
    process.exitCode = 1;
  });
}
