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
  const $ = (id) => document.getElementById(id);
  const controls = ['refresh', 'checkout', 'add', 'sync', 'validate', 'revert', 'submit'];
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
    $('message').style.color = isError ? '#efaaa3' : '';
  }

  function setStatus(file) {
    const badge = $('status');
    badge.className = 'status neutral';
    if (!file) {
      badge.textContent = 'Not tracked';
      return;
    }
    if (file.lock_state === 'other') {
      badge.textContent = 'Checked out by another artist';
      badge.className = 'status bad';
    } else if (file.pending_action) {
      badge.textContent = `${file.pending_action} ready to submit`;
      badge.className = 'status warn';
    } else if (file.needs_sync) {
      badge.textContent = 'New version available';
      badge.className = 'status warn';
    } else if (file.local_state === 'untracked') {
      badge.textContent = 'Not tracked';
    } else {
      badge.textContent = file.lock_state === 'mine' ? 'Checked out by you' : 'Available to edit';
      badge.className = 'status good';
    }
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
    if (!client || !activePath && !['sync', 'submit'].includes(command)) {
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

  configure();
  refresh();
})();
