'use strict';

(function () {
  const path = require('path');
  const bridgeModule = (() => {
    try {
      return require('../lib/openasset-cli');
    } catch (_) {
      return require('../../../common/node/openasset-cli');
    }
  })();
  const { OpenAssetCli } = bridgeModule;
  const { STATUSES, statusFromBridge } = (() => {
    try {
      return require('../lib/words');
    } catch (_) {
      return require('../../../common/node/words');
    }
  })();
  // Which badge tone each canonical status is drawn in. Follows STATUS_COLORS
  // in plugins/common/openasset_depot_bridge/theme.py, except that the shared
  // stylesheet has no accent badge, so "Checked Out" borrows the good tone.
  const STATUS_TONES = {
    [STATUSES.up_to_date]: 'good',
    [STATUSES.checked_out]: 'good',
    [STATUSES.needs_sync]: 'warn',
    [STATUSES.ready_to_submit]: 'warn',
    [STATUSES.in_use]: 'bad',
    [STATUSES.blocked]: 'bad',
    [STATUSES.marked_for_delete]: 'bad',
  };
  const $ = (id) => document.getElementById(id);
  const controls = ['refresh', 'checkout', 'add', 'sync', 'validate', 'revert', 'submit', 'shelve', 'unshelve'];
  let activePath = '';
  let client = null;

  function evalHost(script) {
    return new Promise((resolve) => window.__adobe_cep__.evalScript(script, resolve));
  }

  function settings() {
    return {
      workspaceRoot: localStorage.getItem('oad.workspaceRoot') || '',
      cliPath: localStorage.getItem('oad.cliPath') || 'oad',
    };
  }

  function configure() {
    const value = settings();
    $('workspace-root').value = value.workspaceRoot;
    $('cli-path').value = value.cliPath;
    $('setup').classList.toggle('hidden', Boolean(value.workspaceRoot));
    if (!value.workspaceRoot) {
      client = null;
      return false;
    }
    try {
      client = new OpenAssetCli(value);
      return true;
    } catch (error) {
      showMessage(error.message, true);
      client = null;
      return false;
    }
  }

  function busy(value) {
    controls.forEach((id) => { $(id).disabled = value; });
  }

  function showMessage(message, isError) {
    $('message').textContent = message;
    $('message').style.color = isError ? 'var(--oad-red)' : '';
  }

  function setStatus(file) {
    const badge = $('status');
    if (!file) {
      badge.className = 'status';
      badge.textContent = STATUSES.new_file;
      return;
    }
    const label = statusFromBridge(file);
    badge.className = `status ${STATUS_TONES[label] || ''}`.trimEnd();
    badge.textContent = label;
  }

  async function refresh() {
    if (!configure()) return;
    busy(true);
    try {
      activePath = await evalHost('OpenAssetDepot.currentDocumentPath()');
      const host = await evalHost('OpenAssetDepot.hostName()');
      $('host-name').textContent = host || 'Creative Cloud';
      if (!activePath) {
        $('file-name').textContent = 'No saved document';
        $('file-path').textContent = 'Save the document inside an OpenAsset workspace.';
        setStatus(null);
        showMessage('Waiting for a saved document', false);
        return;
      }
      $('file-name').textContent = path.basename(activePath);
      $('file-path').textContent = activePath;
      await client.run('context');
      const result = await client.run('status', { paths: [activePath] });
      setStatus(result.files && result.files[0]);
      showMessage('Workspace connected', false);
    } catch (error) {
      setStatus(null);
      showMessage(error.message, true);
    } finally {
      busy(false);
    }
  }

  async function run(command, options) {
    if (!client || !activePath && !['sync', 'submit', 'shelve', 'unshelve'].includes(command)) {
      showMessage('Open a saved document in the workspace first', true);
      return;
    }
    busy(true);
    try {
      const result = await client.run(command, options || { path: activePath });
      showMessage(`${command.charAt(0).toUpperCase()}${command.slice(1)} complete`, false);
      await refresh();
      return result;
    } catch (error) {
      showMessage(error.message, true);
    } finally {
      busy(false);
    }
  }

  $('save-settings').addEventListener('click', () => {
    localStorage.setItem('oad.workspaceRoot', $('workspace-root').value.trim());
    localStorage.setItem('oad.cliPath', $('cli-path').value.trim() || 'oad');
    configure();
    refresh();
  });
  $('settings-link').addEventListener('click', () => $('setup').classList.toggle('hidden'));
  $('refresh').addEventListener('click', refresh);
  $('checkout').addEventListener('click', () => run('checkout', { path: activePath, reason: 'Adobe creative edit' }));
  $('add').addEventListener('click', () => run('add', { path: activePath }));
  $('sync').addEventListener('click', () => run('sync', {}));
  $('validate').addEventListener('click', () => run('validate', { paths: [activePath], adapter: 'adobe' }));
  $('revert').addEventListener('click', () => run('revert', { path: activePath }));
  $('submit').addEventListener('click', () => run('submit', {
    description: $('description').value.trim() || 'Submitted from Adobe Creative Cloud',
  }));
  $('shelve').addEventListener('click', () => run('shelve', {}));
  $('unshelve').addEventListener('click', () => run('unshelve', {}));

  configure();
  refresh();
})();
