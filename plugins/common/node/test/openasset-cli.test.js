'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { OpenAssetCli, buildArguments, parseEnvelope } = require('../openasset-cli');

test('buildArguments preserves paths as individual process arguments', () => {
  assert.deepEqual(buildArguments('checkout', { path: 'shots/space scene.blend', reason: 'Lighting pass' }), [
    'integration',
    'checkout',
    'shots/space scene.blend',
    '--reason',
    'Lighting pass',
  ]);
});

test('parseEnvelope rejects protocol mismatches', () => {
  assert.throws(
    () => parseEnvelope('{"protocol_version":2,"ok":true,"data":{}}'),
    /unsupported integration response/
  );
});

test('single-file commands reject ambiguous multi-selection', () => {
  assert.throws(
    () => buildArguments('checkout', { paths: ['A.psd', 'B.psd'] }),
    /requires exactly one file path/
  );
});

test('runner disables shell execution and applies output bounds', async () => {
  let invocation;
  const fakeExec = (binary, args, options, callback) => {
    invocation = { binary, args, options };
    callback(null, '{"protocol_version":1,"ok":true,"data":{"files":[]}}', '');
  };
  const client = new OpenAssetCli(
    { workspaceRoot: '/studio/project', cliPath: '/opt/openasset/oad' },
    { execFile: fakeExec }
  );
  const result = await client.run('status', { paths: ['Assets/Hero.fbx'] });

  assert.deepEqual(result, { files: [] });
  assert.equal(invocation.options.shell, false);
  assert.equal(invocation.options.maxBuffer, 4 * 1024 * 1024);
  assert.deepEqual(invocation.args, [
    '--cwd',
    '/studio/project',
    'integration',
    'status',
    'Assets/Hero.fbx',
  ]);
});
