'use strict';

const { contextBridge, ipcRenderer } = require('electron/renderer');

contextBridge.exposeInMainWorld('openAssetDepot', {
  getSettings: () => ipcRenderer.invoke('oad:settings:get'),
  saveSettings: (settings) => ipcRenderer.invoke('oad:settings:save', settings),
  chooseWorkspace: () => ipcRenderer.invoke('oad:settings:chooseWorkspace'),
  words: () => ipcRenderer.invoke('oad:words'),
  selectedMedia: () => ipcRenderer.invoke('oad:resolve:selectedMedia'),
  run: (command, options) => ipcRenderer.invoke('oad:command', command, options),
});
