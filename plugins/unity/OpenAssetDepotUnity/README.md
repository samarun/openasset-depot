# OpenAsset Depot for Unity

Install this folder as a local Unity Package Manager package. Open **Window > OpenAsset Depot** and configure the optional CLI path under **Project Settings > OpenAsset Depot**.

The package provides:

- Selected asset and active scene status.
- Check Out, Add with `.meta`, Sync Latest, Validate, Submit, and Revert.
- Pre-save checkout for lock-required Unity assets.
- Pre-delete tombstone creation.
- Move tracking as delete-old/add-new in one pending changelist.
- Project window state indicators for recently queried assets.

Unity 2022.3 LTS or newer is supported. The project root must be an OpenAsset workspace and the `oad` CLI must already be authenticated.
