# File Type Rules

Rule precedence:

1. Exact path.
2. Directory prefix.
3. Extension.
4. Fallback default.

Glob rules are preferred for simple path patterns. Regex rules are supported but compiled at startup and should be used only when glob cannot express the match.

Seeded MVP lock-required rules include:

- Unreal: `.uasset`, `.umap`
- Unity: `.unity`, `.prefab`
- Blender: `.blend`
- Maya: `.ma`, `.mb`
- Houdini: `.hip`, `.hda`
- Adobe: `.psd`, `.psb`, `.prproj`, `.aep`
- DaVinci Resolve: `.drp`

Large binary/media assets such as `.fbx`, `.usd`, `.abc`, `.exr`, `.mov`, and `.wav` are classified for locking support and chunked storage.

Generated cache directory rules include `DerivedDataCache/`, `Intermediate/`, `Library/`, `Temp/`, and `Cache/`.
