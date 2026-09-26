'use strict';

const { execFile } = require('child_process');
const path = require('path');

const PROTOCOL_VERSION = 2;
const MAX_OUTPUT_BYTES = 4 * 1024 * 1024;
const DEFAULT_TIMEOUT_MS = 120000;
const LONG_OPERATION_TIMEOUT_MS = 30 * 60 * 1000;
const COMMANDS = new Set([
  'context',
  'pending',
  'status',
  'checkout',
  'add',
  'delete',
  'lock',
  'unlock',
  'revert',
  'sync',
  'submit',
  'shelve',
  'unshelve',
  'history',
  'validate',
]);
const SINGLE_PATH_COMMANDS = new Set(['checkout', 'add', 'delete', 'lock', 'unlock', 'revert', 'history']);
const MULTI_PATH_COMMANDS = new Set(['status', 'validate']);

class OpenAssetCliError extends Error {
  constructor(message, code, details) {
    super(message);
    this.name = 'OpenAssetCliError';
    this.code = code || 'cli_error';
    this.details = details || null;
  }
}

function requireText(value, name, maxLength) {
  if (typeof value !== 'string' || value.length === 0 || value.includes('\0')) {
    throw new OpenAssetCliError(`${name} must be a non-empty string`, 'invalid_input');
  }
  if (value.length > maxLength) {
    throw new OpenAssetCliError(`${name} is too long`, 'invalid_input');
  }
  return value;
}

function normalizeSettings(settings) {
  const candidate = settings || {};
  const workspaceRoot = path.resolve(requireText(candidate.workspaceRoot, 'workspace root', 4096));
  if (!path.isAbsolute(workspaceRoot)) {
    throw new OpenAssetCliError('workspace root must be absolute', 'invalid_input');
  }
  const cliPath = requireText(candidate.cliPath || process.env.OAD_CLI || 'oad', 'CLI path', 4096);
  return { workspaceRoot, cliPath };
}

function buildArguments(command, options) {
  if (!COMMANDS.has(command)) {
    throw new OpenAssetCliError(`unsupported integration command: ${command}`, 'invalid_command');
  }
  const input = options || {};
  // The CLI still defaults to protocol 1, so the version has to be requested.
  const args = ['integration', '--protocol-version', String(PROTOCOL_VERSION), command];
  const paths = Array.isArray(input.paths) ? input.paths : input.path ? [input.path] : [];
  if (SINGLE_PATH_COMMANDS.has(command) && paths.length !== 1) {
    throw new OpenAssetCliError(`${command} requires exactly one file path`, 'invalid_input');
  }
  if (!SINGLE_PATH_COMMANDS.has(command) && !MULTI_PATH_COMMANDS.has(command) && paths.length > 0) {
    throw new OpenAssetCliError(`${command} does not accept file paths`, 'invalid_input');
  }
  for (const filePath of paths) {
    args.push(requireText(filePath, 'file path', 4096));
  }
  if (input.reason && !['checkout', 'delete', 'lock'].includes(command)) {
    throw new OpenAssetCliError(`${command} does not accept a reason`, 'invalid_input');
  }
  if (input.reason) {
    args.push('--reason', requireText(input.reason, 'reason', 1024));
  }
  if (input.adapter && command !== 'validate') {
    throw new OpenAssetCliError(`${command} does not accept an adapter`, 'invalid_input');
  }
  if (input.adapter) {
    args.push('--adapter', requireText(input.adapter, 'adapter', 128));
  }
  if (input.description && command !== 'submit') {
    throw new OpenAssetCliError(`${command} does not accept a description`, 'invalid_input');
  }
  if (input.description) {
    args.push('--description', requireText(input.description, 'description', 4096));
  }
  return args;
}

/**
 * Reads the protocol 2 stream, which is one JSON object per line: any number of
 * `progress` lines followed by exactly one `result` line.
 *
 * These panels run the CLI through `execFile`, which buffers stdout and hands it
 * over once the process exits, so there is no live progress to report and the
 * progress lines are only skipped. A host that wants a progress bar has to spawn
 * the CLI itself, the way `openasset_depot_bridge.client` does.
 */
function parseEnvelope(stdout) {
  const lines = String(stdout).split('\n').map((line) => line.trim()).filter(Boolean);
  let envelope = null;
  for (const line of lines) {
    let payload;
    try {
      payload = JSON.parse(line);
    } catch (error) {
      throw new OpenAssetCliError('oad returned invalid JSON', 'invalid_response', error.message);
    }
    if (!payload || payload.protocol_version !== PROTOCOL_VERSION) {
      throw new OpenAssetCliError('oad returned an unsupported integration response', 'protocol_mismatch');
    }
    if (payload.type === 'progress') {
      continue;
    }
    if (payload.type !== 'result' || typeof payload.ok !== 'boolean') {
      throw new OpenAssetCliError('oad returned an unsupported integration response', 'protocol_mismatch');
    }
    envelope = payload;
  }
  if (!envelope) {
    throw new OpenAssetCliError('oad returned no integration result', 'invalid_response');
  }
  if (!envelope.ok) {
    throw new OpenAssetCliError(envelope.error || 'oad command failed', 'command_failed');
  }
  return envelope.data;
}

class OpenAssetCli {
  constructor(settings, executionOptions) {
    this.settings = normalizeSettings(settings);
    const options = executionOptions || {};
    this.timeoutMs = options.timeoutMs || DEFAULT_TIMEOUT_MS;
    this.execFile = options.execFile || execFile;
  }

  run(command, options) {
    const args = ['--cwd', this.settings.workspaceRoot, ...buildArguments(command, options)];
    const timeoutMs = ['sync', 'submit', 'shelve', 'unshelve'].includes(command)
      ? Math.max(this.timeoutMs, LONG_OPERATION_TIMEOUT_MS)
      : this.timeoutMs;
    return new Promise((resolve, reject) => {
      this.execFile(
        this.settings.cliPath,
        args,
        {
          cwd: this.settings.workspaceRoot,
          windowsHide: true,
          timeout: timeoutMs,
          maxBuffer: MAX_OUTPUT_BYTES,
          shell: false,
          encoding: 'utf8',
        },
        (error, stdout, stderr) => {
          if (error) {
            const timedOut = Boolean(error.killed) || error.code === 'ETIMEDOUT';
            const detail = String(stderr || '').trim();
            reject(
              new OpenAssetCliError(
                timedOut ? 'oad command timed out' : detail || error.message,
                timedOut ? 'timeout' : 'process_failed',
                { exitCode: error.code || null }
              )
            );
            return;
          }
          try {
            resolve(parseEnvelope(String(stdout || '').trim()));
          } catch (parseError) {
            reject(parseError);
          }
        }
      );
    });
  }
}

module.exports = {
  OpenAssetCli,
  OpenAssetCliError,
  buildArguments,
  normalizeSettings,
  parseEnvelope,
};
