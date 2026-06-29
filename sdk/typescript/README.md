# OpenAsset Depot TypeScript SDK

This package defines the MVP adapter/client contract for desktop, web, and native bridge integrations.

```ts
import { OpenAssetClient } from "@openasset/depot-sdk";

const client = new OpenAssetClient("http://127.0.0.1:8080");
await client.login("alice", "correct horse battery staple");
const adapters = await client.listAdapters();
```

The SDK mirrors the Rust contract and keeps DCC-specific behavior behind adapter endpoints:

- `GET /api/adapters`
- `POST /api/adapters/detect`
- `POST /api/adapters/scan`
- `POST /api/adapters/preview`
- `POST /api/adapters/metadata`
- `POST /api/adapters/validate`
