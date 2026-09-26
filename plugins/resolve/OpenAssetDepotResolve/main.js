'use strict';

const { app, BrowserWindow, dialog, ipcMain } = require('electron');
const fs = require('fs');
const path = require('path');
const WorkflowIntegration = require('./WorkflowIntegration.node');
const { OpenAssetCli } = (() => {
  try {
    return require('./lib/openasset-cli');
  } catch (_) {
    return require('../../common/node/openasset-cli');
  }
})();
const words = (() => {
  try {
    return require('./lib/words');
  } catch (_) {
    return require('../../common/node/words');
  }
})();

const PLUGIN_ID = 'com.openassetdepot.resolve';
const SETTINGS_FILE = 'settings.json';
let mainWindow = null;
let resolveObject = null;

function settingsPath() {
  return path.join(app.getPath('userData'), SETTINGS_FILE);
}

function loadSettings() {
  try {
    const parsed = JSON.parse(fs.readFileSync(settingsPath(), 'utf8'));
    return { workspaceRoot: parsed.workspaceRoot || '', cliPath: parsed.cliPath || 'oad' };
  } catch (_) {
    return { workspaceRoot: '', cliPath: 'oad' };
  }
}

function saveSettings(_event, input) {
  const workspaceRoot = String(input.workspaceRoot || '').trim();
  if (!workspaceRoot || workspaceRoot.includes('\0')) {
    throw new Error('Workspace root must be an absolute path');
  }
  const settings = {
    workspaceRoot: path.resolve(workspaceRoot),
    cliPath: String(input.cliPath || 'oad'),
  };
  if (!path.isAbsolute(settings.workspaceRoot) || settings.workspaceRoot.includes('\0')) {
    throw new Error('Workspace root must be an absolute path');
  }
  fs.mkdirSync(path.dirname(settingsPath()), { recursive: true });
  const temporary = `${settingsPath()}.${process.pid}.tmp`;
  fs.writeFileSync(temporary, JSON.stringify(settings, null, 2), { encoding: 'utf8', mode: 0o600 });
  fs.renameSync(temporary, settingsPath());
  return settings;
}

async function initializeResolve() {
  if (resolveObject) return resolveObject;
  WorkflowIntegration.SetAPITimeout(30);
  const initialized = await WorkflowIntegration.InitializePromise(PLUGIN_ID);
  if (!initialized) throw new Error('Resolve workflow API initialization failed');
  resolveObject = await WorkflowIntegration.GetResolvePromise();
  if (!resolveObject) throw new Error('Resolve is not available');
  return resolveObject;
}

async function selectedMedia() {
  const resolve = await initializeResolve();
  const manager = await resolve.GetProjectManager();
  const project = manager && await manager.GetCurrentProject();
  if (!project) return { projectName: '', files: [] };
  const pool = await project.GetMediaPool();
  const clips = pool ? await pool.GetSelectedClips() : [];
  const files = [];
  for (const clip of clips || []) {
    const properties = await clip.GetClipProperty();
    const filePath = properties && (properties['File Path'] || properties.FilePath);
    if (typeof filePath === 'string' && filePath.length > 0 && !filePath.includes('\0')) {
      files.push({ name: await clip.GetName(), path: filePath });
    }
  }
  const unique = new Map(files.map((file) => [file.path, file]));
  return { projectName: await project.GetName(), files: Array.from(unique.values()) };
}

async function chooseWorkspace() {
  const result = await dialog.showOpenDialog(mainWindow, {
    title: 'Choose OpenAsset workspace',
    properties: ['openDirectory'],
  });
  return result.canceled ? null : result.filePaths[0];
}

// The renderer is sandboxed with context isolation, so it cannot require the
// shared vocabulary the way the Adobe panel does. Only the label tables cross
// the IPC boundary; the helper functions would not survive the structured
// clone, so the renderer composes its badge text from STATUSES.
function vocabulary() {
  return {
    PRODUCT_NAME: words.PRODUCT_NAME,
    ACTIONS: words.ACTIONS,
    STATUSES: words.STATUSES,
    FIELDS: words.FIELDS,
    MESSAGES: words.MESSAGES,
  };
}

async function runCommand(_event, command, options) {
  const allowed = new Set(['context', 'status', 'checkout', 'add', 'sync', 'validate', 'revert', 'submit', 'shelve', 'unshelve']);
  if (!allowed.has(command)) throw new Error(`Unsupported command: ${command}`);
  return new OpenAssetCli(loadSettings()).run(command, options || {});
}

function registerHandlers() {
  ipcMain.handle('oad:settings:get', loadSettings);
  ipcMain.handle('oad:settings:save', saveSettings);
  ipcMain.handle('oad:settings:chooseWorkspace', chooseWorkspace);
  ipcMain.handle('oad:words', vocabulary);
  ipcMain.handle('oad:resolve:selectedMedia', selectedMedia);
  ipcMain.handle('oad:command', runCommand);
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 430,
    height: 680,
    minWidth: 380,
    minHeight: 520,
    useContentSize: true,
    backgroundColor: '#191a1c',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });
  mainWindow.removeMenu();
  mainWindow.loadFile('index.html');
  mainWindow.on('closed', () => { mainWindow = null; });
}

app.whenReady().then(() => {
  registerHandlers();
  createWindow();
});

app.on('before-quit', () => {
  if (resolveObject) WorkflowIntegration.CleanUp();
  resolveObject = null;
});

app.on('window-all-closed', () => app.quit());
