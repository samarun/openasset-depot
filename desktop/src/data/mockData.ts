import type {
  AssetFile,
  Changelist,
  Depot,
  FileTypeRule,
  LockInfo,
  Review,
  StageState,
  ValidationMessage,
  Workspace,
} from "../types/domain";

export const mockDepots: Depot[] = [
  {
    id: "demo-depot",
    name: "Demo Creative Depot",
    description: "Mock-marked local review data for the desktop UI.",
  },
];

export const mockWorkspace: Workspace = {
  id: "demo-workspace",
  depot_id: "demo-depot",
  stream_id: "main",
  name: "Demo Production Workspace",
  local_path: "/Projects/OpenAssetDemo",
};

export const mockFiles: AssetFile[] = [
  {
    id: "file-map-main",
    name: "Main.umap",
    path: "Content/Maps/Main.umap",
    kind: "Unreal map",
    size: "248 MB",
    revision: 42,
    owner: "maya.chen",
    statuses: ["Checked Out", "Ready to Submit"],
    previewTone: "scene",
    dependencies: ["LightingRig.uasset", "HeroShip.uasset", "Main.umap.meta"],
    changelist: "Lighting polish",
    source: "mock",
  },
  {
    id: "file-hero-ship",
    name: "HeroShip.uasset",
    path: "Content/Vehicles/HeroShip.uasset",
    kind: "Unreal asset",
    size: "91 MB",
    revision: 18,
    owner: "td.ravi",
    statuses: ["In Use", "Needs Sync"],
    previewTone: "model",
    dependencies: ["HeroShip_LOD0.fbx", "ShipPaint.psd"],
    source: "mock",
  },
  {
    id: "file-scene",
    name: "IntroScene.unity",
    path: "Assets/Scenes/IntroScene.unity",
    kind: "Unity scene",
    size: "12 MB",
    revision: 11,
    statuses: ["Up to Date"],
    previewTone: "scene",
    dependencies: ["IntroScene.unity.meta", "Environment.prefab"],
    source: "mock",
  },
  {
    id: "file-comp",
    name: "Shot010.nk",
    path: "Comp/Shot010.nk",
    kind: "Nuke script",
    size: "740 KB",
    revision: 7,
    statuses: ["Ready to Submit"],
    previewTone: "document",
    dependencies: ["Shot010_plate.exr"],
    changelist: "Comp cleanup",
    source: "mock",
  },
  {
    id: "file-audio",
    name: "Ambience.wav",
    path: "Audio/Ambience.wav",
    kind: "Wave audio",
    size: "63 MB",
    revision: 5,
    statuses: ["New File"],
    previewTone: "audio",
    dependencies: [],
    changelist: "Stage audio",
    source: "mock",
  },
];

export const mockWarnings: ValidationMessage[] = [
  {
    path: "Content/Maps/Main.umap",
    code: "unity_meta_missing",
    message: "Include the matching metadata file before submit.",
  },
  {
    path: "DerivedDataCache/ShaderCache.bin",
    code: "generated_cache",
    message: "Remove generated cache files from the changelist.",
  },
];

export const mockChangelists: Changelist[] = [
  {
    id: "cl-lighting-polish",
    title: "Lighting polish",
    description: "Map lighting update for the Friday review.",
    files: [mockFiles[0], mockFiles[4]],
    warnings: mockWarnings,
    ready: false,
    source: "mock",
  },
  {
    id: "cl-comp-cleanup",
    title: "Comp cleanup",
    description: "Nuke script tidy and plate path fix.",
    files: [mockFiles[3]],
    warnings: [],
    ready: true,
    source: "mock",
  },
];

export const mockReviews: Review[] = [
  {
    id: "review-stage-12",
    title: "Stage rehearsal package",
    status: "Waiting",
    owner: "maya.chen",
    files: 12,
    source: "mock",
  },
  {
    id: "review-ship-lookdev",
    title: "Hero ship lookdev",
    status: "Approved",
    owner: "td.ravi",
    files: 6,
    source: "mock",
  },
];

export const mockLocks: LockInfo[] = [
  {
    depot_path: "Content/Vehicles/HeroShip.uasset",
    user_id: "td.ravi",
    reason: "Lookdev pass",
    state: "active",
    created_at: new Date(Date.now() - 1000 * 60 * 78).toISOString(),
    source: "mock",
  },
  {
    depot_path: "Assets/Scenes/IntroScene.unity",
    user_id: "maya.chen",
    reason: "Blocking pass",
    state: "active",
    created_at: new Date(Date.now() - 1000 * 60 * 18).toISOString(),
    source: "mock",
  },
];

export const mockFiletypes: FileTypeRule[] = [
  {
    name: "Unreal map package",
    rule_kind: "extension",
    extension: ".umap",
    asset_class: "unreal_map",
    lock_required: true,
    generated: false,
    source: "mock",
  },
  {
    name: "Unity metadata",
    rule_kind: "extension",
    extension: ".meta",
    asset_class: "unity_metadata",
    lock_required: false,
    generated: false,
    source: "mock",
  },
  {
    name: "Generated cache",
    rule_kind: "directory",
    directory_prefix: "DerivedDataCache/",
    asset_class: "generated_cache",
    lock_required: false,
    generated: true,
    source: "mock",
  },
];

export const mockStage: StageState = {
  stageRevision: "main@2481",
  liveApproval: "Pending",
  renderNodes: [
    { name: "vp-wall-01", status: "Up to Date", lastSync: "2 min ago" },
    { name: "vp-wall-02", status: "Needs Sync", lastSync: "26 min ago" },
    { name: "lighting-preview", status: "Up to Date", lastSync: "Just now" },
  ],
  validations: [
    {
      path: "Content/Maps/Main.umap",
      code: "stage_validation",
      message: "Sync the remaining render node before marking live approved.",
    },
  ],
  source: "mock",
};
