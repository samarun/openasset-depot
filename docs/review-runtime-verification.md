# Review Runtime Verification

Verified on 2026-09-26 against the Rust API, PostgreSQL, and the React review
studio in system Chrome controlled by Playwright.

## Fixture

Blender 5.1.1 generated a two-second animated cube at 24 fps and exported it as
both binary glTF (`.glb`) and FBX 7400. The files were submitted through the
browser workflow as immutable version 1 revisions.

## Passing Behaviors

- A GLB revision decodes, frames the model, and plays its animation.
- An FBX revision decodes, frames the model, and plays its animation.
- A GLB review proxy attached to the FBX revision is selected by served content
  type and decoded as GLB rather than being sent to the FBX loader.
- Pen annotations and timecoded comments are stored by the API and survive
  closing and reopening the review.
- Selecting the stored timecode seeks the animation and pauses on that frame.
- Looping playback reports the active clip's local time and never produces a
  comment time beyond the clip duration.
- Review media and proxy downloads support HTTP byte ranges.
- The completed flow produces no browser page errors or console errors.
- Committed Apache-2.0 project fixtures exercise GLB, embedded glTF, and FBX in
  headless Chromium. CI requires successful decoding and a non-background
  WebGL canvas; animated GLB/FBX fixtures must also advance their clip time.
- Malformed exporter tracks are isolated and discarded without collapsing an
  otherwise playable FBX scene.
- Review proxies use resumable upload sessions and preserve rational source
  rates such as `24000/1001` plus the source timeline start frame.

The API integration suite separately verifies immutable proxy conflicts,
annotation JSON, comment resolution, range responses, and authorization.

## Current Boundaries

- Legacy review proxies without source timebase metadata show elapsed time but
  intentionally omit frame labels.
- Browser-native review supports GLB, glTF, FBX, MP4, WebM, common images, and
  common audio. USD, Alembic, EXR sequences, and native DCC scenes need generated
  review proxies.
- Browser CI uses Chromium software WebGL; periodic validation on representative
  studio GPUs remains valuable for vendor-specific driver behavior.
