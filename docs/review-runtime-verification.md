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

The API integration suite separately verifies immutable proxy conflicts,
annotation JSON, comment resolution, range responses, and authorization.

## Current Boundaries

- Frame labels currently assume 24 fps. Source frame-rate/timebase metadata is
  not yet stored with a revision or proxy.
- Review-proxy uploads are capped at 512 MiB and currently buffer the request in
  memory. They should move to the resumable streaming upload service before
  accepting feature-length media or dense production geometry.
- Browser-native review supports GLB, glTF, FBX, MP4, WebM, common images, and
  common audio. USD, Alembic, EXR sequences, and native DCC scenes need generated
  review proxies.
- GPU/browser behavior is verified manually with the repeatable flow above; the
  repository still needs a licensed, committed fixture set and canvas-pixel CI.
