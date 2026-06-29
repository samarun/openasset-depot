# OpenAsset Depot Desktop

This is the first desktop UI for OpenAsset Depot. It is a Tauri + React + TypeScript app that uses the existing REST backend and keeps demo fixtures isolated to an explicit offline fallback mode.

## Run

```sh
cd desktop
npm install
npm run dev
```

Open the Vite URL for browser development, or run the Tauri shell:

```sh
npm run tauri dev
```

The login screen defaults to the current origin for browser development. Set another server with:

```sh
VITE_OPENASSET_API_URL=http://127.0.0.1:18080 npm run dev
```

## Verify

```sh
npm run build
npm test
```

## Backend-Backed Screens

- Login calls `/api/auth/login`.
- Workspace selector calls `/api/workspaces`.
- Admin calls `/api/depots` and `/api/filetypes`.
- Lock Center calls `/api/locks`.
- Workspace assets call `/api/sync/plan`.
- Lock, unlock, add, and revert actions call `/api/files/*`.
- File history calls `/api/files/history`.
- Submit validation calls `/api/validate`.

## Demo Mode

When a signed-in client cannot reach the server, the UI falls back to demo fixtures and shows `Demo Mode · Server not connected`. Connected sessions never show mock assets or badges. Reviews, stage state, and local changelist composition remain future backend/native-integration surfaces and display honest empty states while connected.
