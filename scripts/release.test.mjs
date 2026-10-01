import assert from 'node:assert/strict';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import {
  CRATES,
  ROOT,
  crateVersions,
  findMergedRcPr,
  nextRc,
  plan,
  prepare,
  prCheck,
  rateLimitRetryAt,
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

test('crate versions follow crates.io seek pagination without a page number', async (context) => {
  const previousFetch = globalThis.fetch;
  const requested = [];
  globalThis.fetch = async (url) => {
    requested.push(url);
    if (new URL(url).searchParams.has('page')) {
      return new Response(JSON.stringify({ errors: [{ detail: '?page= is not supported for this request' }] }), { status: 400 });
    }
    if (url.includes('seek=older')) {
      return new Response(JSON.stringify({
        versions: [{ num: '0.1.0-rc.1' }],
        meta: { next_page: null },
      }), { status: 200 });
    }
    return new Response(JSON.stringify({
      versions: [{ num: '0.1.0-rc.2' }],
      meta: { next_page: '?seek=older&per_page=100' },
    }), { status: 200 });
  };
  context.after(() => { globalThis.fetch = previousFetch; });
  assert.deepEqual([...await crateVersions('tilt-ui-icons')], ['0.1.0-rc.2', '0.1.0-rc.1']);
  assert.deepEqual(requested, [
    'https://crates.io/api/v1/crates/tilt-ui-icons/versions?per_page=100',
    'https://crates.io/api/v1/crates/tilt-ui-icons/versions?seek=older&per_page=100',
  ]);
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

test('a partial RC resumes the same version and source commit', async (context) => {
  const root = mkdtempSync(join(tmpdir(), 'tilt-ui-release-'));
  context.after(() => rmSync(root, { recursive: true }));
  const output = join(root, 'output');
  setEnvironment(context, {
    GITHUB_REF_NAME: 'main',
    GITHUB_REPOSITORY: 'owner/repo',
    GITHUB_OUTPUT: output,
    RELEASE_RESUME_VERSION: '0.1.0-rc.1',
    RELEASE_SOURCE_SHA: 'a'.repeat(40),
  });
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    if (url.includes('/crates/tilt-ui-core/versions')) {
      return new Response(JSON.stringify({ versions: [{ num: '0.1.0-rc.1' }] }), { status: 200 });
    }
    if (url.includes('crates.io')) {
      return new Response(JSON.stringify({ versions: [] }), { status: 200 });
    }
    return new Response('', { status: 404 });
  };
  context.after(() => { globalThis.fetch = previousFetch; });
  await plan();
  assert.equal(readFileSync(output, 'utf8'),
    'publish=true\nversion=0.1.0-rc.1\ntag=v0.1.0-rc.1\nkind=rc\n');
});

test('crates.io retry time is parsed only from a rate limit response', () => {
  const now = Date.parse('2026-09-30T19:00:00Z');
  const response = 'status 429 Too Many Requests: Please try again after Wed, 30 Sep 2026 20:02:49 GMT and see https://crates.io/docs/rate-limits';
  assert.equal(rateLimitRetryAt(response, now), Date.parse('2026-09-30T20:03:04Z'));
  assert.equal(rateLimitRetryAt('status 500: Please try again after Wed, 30 Sep 2026 20:02:49 GMT and see docs', now), null);
});

test('publish skips completed crates and uploads only the remaining crates in order', async (context) => {
  const root = mkdtempSync(join(tmpdir(), 'tilt-ui-publish-'));
  context.after(() => rmSync(root, { recursive: true }));
  const bin = join(root, 'bin');
  const calls = join(root, 'cargo-calls');
  mkdirSync(bin);
  writeFileSync(join(root, 'Cargo.toml'), '[workspace.package]\nversion = "0.1.0-rc.1"\n[workspace.dependencies]\n');
  writeFileSync(join(bin, 'cargo'), '#!/usr/bin/env node\nrequire("node:fs").appendFileSync(process.env.CARGO_CALLS_FILE, process.argv.slice(2).join(" ") + "\\n");\n');
  chmodSync(join(bin, 'cargo'), 0o755);
  setEnvironment(context, {
    TILT_UI_RELEASE_ROOT: root,
    CARGO_CALLS_FILE: calls,
    CARGO_REGISTRY_TOKEN: 'test-token',
    PATH: `${bin}:${process.env.PATH}`,
  });
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    const name = url.match(/\/crates\/(tilt-ui(?:-[a-z]+)?)\/versions/)?.[1];
    const versions = CRATES.indexOf(name) < 5 ? [{ num: '0.1.0-rc.1' }] : [];
    return new Response(JSON.stringify({ versions }), { status: 200 });
  };
  context.after(() => { globalThis.fetch = previousFetch; });
  const { publish } = await import('./release.mjs?publish-resume-test');
  await publish('0.1.0-rc.1');
  assert.deepEqual(readFileSync(calls, 'utf8').trim().split('\n'), [
    'package --package tilt-ui-build --locked --no-verify',
    'publish --package tilt-ui-build --locked --no-verify',
    'package --package tilt-ui-runtime --locked --no-verify',
    'publish --package tilt-ui-runtime --locked --no-verify',
    'package --package tilt-ui --locked --no-verify',
    'publish --package tilt-ui --locked --no-verify',
  ]);
});
