'use strict';

const $ = (id) => document.getElementById(id);
const controls = ['refresh', 'checkout', 'add', 'sync', 'validate', 'revert', 'submit', 'shelve', 'unshelve'];
let files = [];
let configured = false;
let words = null;

function busy(value) { controls.forEach((id) => { $(id).disabled = value; }); }
function message(value, error) { $('message').textContent = value; $('message').style.color = error ? 'var(--oad-red)' : ''; }
function paths() { return files.map((file) => file.path); }

async function loadSettings() {
  const settings = await window.openAssetDepot.getSettings();
  $('workspace-root').value = settings.workspaceRoot || '';
  $('cli-path').value = settings.cliPath || 'oad';
  configured = Boolean(settings.workspaceRoot);
  $('setup').classList.toggle('hidden', configured);
}

// The badge summarises a multi-clip selection, so it reports the status of the
// clip that most needs attention, in the same precedence the shared statusLabel
// uses for a single file.
function displayStatus(result) {
  const entries = result && result.files || [];
  const badge = $('status');
  const { STATUSES } = words;
  badge.className = 'status';
  if (entries.some((entry) => entry.lock_state === 'other')) {
    badge.textContent = STATUSES.in_use; badge.classList.add('bad');
  } else if (entries.some((entry) => entry.pending_action)) {
    badge.textContent = STATUSES.ready_to_submit; badge.classList.add('warn');
  } else if (entries.some((entry) => entry.needs_sync)) {
    badge.textContent = STATUSES.needs_sync; badge.classList.add('warn');
  } else if (entries.length > 0) {
    badge.textContent = STATUSES.up_to_date; badge.classList.add('good');
  } else {
    badge.textContent = STATUSES.new_file;
  }
}

async function refresh() {
  if (!words) words = await window.openAssetDepot.words();
  await loadSettings();
  if (!configured) return;
  busy(true);
  try {
    const selection = await window.openAssetDepot.selectedMedia();
    files = selection.files || [];
    $('project').textContent = selection.projectName || 'DaVinci Resolve';
    $('selection-title').textContent = files.length ? `${files.length} selected ${files.length === 1 ? 'clip' : 'clips'}` : 'No clips selected';
    $('selection-paths').textContent = files.length ? files.map((file) => file.name).join('  |  ') : 'Select one or more Media Pool clips stored in this workspace.';
    await window.openAssetDepot.run('context', {});
    const status = files.length ? await window.openAssetDepot.run('status', { paths: paths() }) : { files: [] };
    displayStatus(status);
    message('Workspace connected', false);
  } catch (error) {
    displayStatus(null); message(error.message, true);
  } finally { busy(false); }
}

async function run(command, options) {
  if (!configured) { message('Connect a workspace first', true); return; }
  if (!files.length && !['sync', 'submit', 'shelve', 'unshelve'].includes(command)) { message('Select Media Pool clips first', true); return; }
  busy(true);
  try {
    await window.openAssetDepot.run(command, options);
    message(`${command.charAt(0).toUpperCase()}${command.slice(1)} complete`, false);
    await refresh();
  } catch (error) { message(error.message, true); }
  finally { busy(false); }
}

async function runForEach(command, optionsForPath) {
  if (!configured) { message('Connect a workspace first', true); return; }
  const selectedPaths = paths();
  if (!selectedPaths.length) { message('Select Media Pool clips first', true); return; }
  busy(true);
  try {
    for (const filePath of selectedPaths) {
      await window.openAssetDepot.run(command, optionsForPath(filePath));
    }
    message(`${command.charAt(0).toUpperCase()}${command.slice(1)} complete for ${selectedPaths.length} ${selectedPaths.length === 1 ? 'clip' : 'clips'}`, false);
    await refresh();
  } catch (error) { message(error.message, true); }
  finally { busy(false); }
}

$('choose').addEventListener('click', async () => { const selected = await window.openAssetDepot.chooseWorkspace(); if (selected) $('workspace-root').value = selected; });
$('save-settings').addEventListener('click', async () => {
  try {
    await window.openAssetDepot.saveSettings({ workspaceRoot: $('workspace-root').value, cliPath: $('cli-path').value || 'oad' });
    await refresh();
  } catch (error) { message(error.message, true); }
});
$('settings-link').addEventListener('click', () => $('setup').classList.toggle('hidden'));
$('refresh').addEventListener('click', refresh);
$('checkout').addEventListener('click', () => runForEach('checkout', (filePath) => ({ path: filePath, reason: 'DaVinci Resolve editorial work' })));
$('add').addEventListener('click', () => runForEach('add', (filePath) => ({ path: filePath })));
$('sync').addEventListener('click', () => run('sync', {}));
$('validate').addEventListener('click', () => run('validate', { paths: paths(), adapter: 'resolve' }));
$('revert').addEventListener('click', () => runForEach('revert', (filePath) => ({ path: filePath })));
$('submit').addEventListener('click', () => run('submit', { description: $('description').value.trim() || 'Submitted from DaVinci Resolve' }));
$('shelve').addEventListener('click', () => run('shelve', {}));
$('unshelve').addEventListener('click', () => run('unshelve', {}));

refresh();
