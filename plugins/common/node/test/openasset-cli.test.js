'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const { OpenAssetCli, buildArguments, parseEnvelope } = require('../openasset-cli');

test('buildArguments preserves paths as individual process arguments', () => {
  assert.deepEqual(buildArguments('checkout', { path: 'shots/space scene.blend', reason: 'Lighting pass' }), [
    'integration',
    '--protocol-version',
    '2',
    'checkout',
    'shots/space scene.blend',
    '--reason',
    'Lighting pass',
  ]);
});

test('parseEnvelope rejects protocol mismatches', () => {
  assert.throws(
    () => parseEnvelope('{"protocol_version":1,"ok":true,"data":{}}'),
    /unsupported integration response/
  );
});

test('parseEnvelope skips progress lines and returns the result payload', () => {
  const stream = [
    '{"protocol_version":2,"type":"progress","operation":"sync","phase":"starting","message":"Starting sync","completed":5,"total":100}',
    '{"protocol_version":2,"type":"progress","operation":"sync","phase":"transfer","message":"Downloading files","completed":60,"total":100,"files_completed":3,"files_total":5}',
    '{"protocol_version":2,"type":"result","ok":true,"data":{"synced_count":5},"error":null}',
    '',
  ].join('\n');

  assert.deepEqual(parseEnvelope(stream), { synced_count: 5 });
});

test('parseEnvelope reports a stream that never reached a result', () => {
  assert.throws(
    () => parseEnvelope('{"protocol_version":2,"type":"progress","operation":"sync","completed":5}'),
    /no integration result/
  );
});

test('single-file commands reject ambiguous multi-selection', () => {
  assert.throws(
    () => buildArguments('checkout', { paths: ['A.psd', 'B.psd'] }),
    /requires exactly one file path/
  );
});

test('workspace commands reject file paths', () => {
  assert.throws(
    () => buildArguments('shelve', { path: 'Assets/Hero.prefab' }),
    /does not accept file paths/
  );
});

test('runner disables shell execution and applies output bounds', async () => {
  let invocation;
  const fakeExec = (binary, args, options, callback) => {
    invocation = { binary, args, options };
    callback(
      null,
      [
        '{"protocol_version":2,"type":"progress","operation":"status","phase":"starting","message":"Starting status","completed":5,"total":100}',
        '{"protocol_version":2,"type":"result","ok":true,"data":{"files":[]},"error":null}',
      ].join('\n'),
      ''
    );
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
    '--protocol-version',
    '2',
    'status',
    'Assets/Hero.fbx',
  ]);
});
