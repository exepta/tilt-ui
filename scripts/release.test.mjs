import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import {
  CRATES,
  ROOT,
  findMergedRcPr,
  nextRc,
  plan,
  prepare,
  prCheck,
  stableCollisions,
  workspaceVersion,
} from './release.mjs';

function emptyRegistry() {
  return Object.fromEntries(CRATES.map((name) => [name, new Set()]));
}

test('prepare sets an exact RC version for every publishable crate', (context) => {
  const root = mkdtempSync(join(tmpdir(), 'tilt-ui-release-'));
  context.after(() => rmSync(root, { recursive: true }));
  writeFileSync(join(root, 'Cargo.toml'), readFileSync(join(ROOT, 'Cargo.toml')));
  prepare('0.1.0-rc.2', root);
  const manifest = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  assert.equal(workspaceVersion(root), '0.1.0-rc.2');
  for (const name of CRATES) {
    assert.ok(manifest.includes(`${name} = { path = "crates/${name}", version = "=0.1.0-rc.2" }`));
  }
});

test('next RC follows the highest GitHub tag or crates.io version', () => {
  const published = emptyRegistry();
  published['tilt-ui-core'].add('0.1.0-rc.2');
  assert.equal(nextRc('0.1.0', published, ['v0.1.0-rc.1', 'v0.1.0-rc.4']), '0.1.0-rc.5');
});

test('only a merged RC: PR targeting main triggers an RC release', () => {
  const pr = { merged_at: 'today', base: { ref: 'main' }, head: { ref: 'feature' } };
  assert.equal(findMergedRcPr([
    { ...pr, number: 1, title: 'Feature: RC: later' },
    { ...pr, number: 2, title: 'RC: preview', base: { ref: 'other' } },
    { ...pr, number: 3, title: 'RC: preview', head: { ref: 'release-0.1.0' } },
    { ...pr, number: 4, title: 'RC: preview' },
  ]), 4);
});

test('stable release guard reports an existing tag or crate version', () => {
  const published = emptyRegistry();
  published['tilt-ui-icons'].add('0.1.0');
  assert.deepEqual(stableCollisions('0.1.0', published, true), [
    'crates.io/tilt-ui-icons',
    'GitHub tag v0.1.0',
  ]);
});

function setEnvironment(context, values) {
  const previous = Object.fromEntries(Object.keys(values).map((key) => [key, process.env[key]]));
  Object.assign(process.env, values);
  context.after(() => {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  });
}

test('a regular main push does not schedule a publication', async (context) => {
  const root = mkdtempSync(join(tmpdir(), 'tilt-ui-release-'));
  context.after(() => rmSync(root, { recursive: true }));
  const output = join(root, 'output');
  setEnvironment(context, {
    GITHUB_REF_NAME: 'main',
    GITHUB_REPOSITORY: 'owner/repo',
    GITHUB_SHA: 'a'.repeat(40),
    GITHUB_OUTPUT: output,
  });
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => new Response('[]', { status: 200 });
  context.after(() => { globalThis.fetch = previousFetch; });
  await plan();
  assert.equal(readFileSync(output, 'utf8'), 'publish=false\n');
});

test('a release PR fails when its version already exists on crates.io', async (context) => {
  const root = mkdtempSync(join(tmpdir(), 'tilt-ui-release-'));
  context.after(() => rmSync(root, { recursive: true }));
  const event = join(root, 'event.json');
  writeFileSync(event, JSON.stringify({
    pull_request: { title: 'Release', head: { ref: 'release-0.1.0' } },
  }));
  setEnvironment(context, {
    GITHUB_EVENT_PATH: event,
    GITHUB_REPOSITORY: 'owner/repo',
  });
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    if (url.includes('/crates/tilt-ui-icons/')) {
      return new Response(JSON.stringify({ versions: [{ num: '0.1.0' }] }), { status: 200 });
    }
    if (url.includes('crates.io')) {
      return new Response(JSON.stringify({ versions: [] }), { status: 200 });
    }
    return new Response('', { status: 404 });
  };
  context.after(() => { globalThis.fetch = previousFetch; });
  await assert.rejects(prCheck(), /v0\.1\.0 already exists: crates\.io\/tilt-ui-icons/);
});
