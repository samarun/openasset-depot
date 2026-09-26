import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { sanitizeAnimationClips } from "./ThreeReviewPlayer";

describe("ThreeReviewPlayer animation safety", () => {
  it("drops malformed exporter tracks without discarding a playable clip", () => {
    const valid = new THREE.VectorKeyframeTrack("Cube.scale", [0, 1], [1, 1, 1, 2, 2, 2]);
    const malformed = new THREE.QuaternionKeyframeTrack("Cube.quaternion", [0], [0, 0, 0, 1]);
    malformed.values = new Float32Array([0]);
    const clips = sanitizeAnimationClips([
      new THREE.AnimationClip("Blender FBX", 1, [valid, malformed]),
    ]);

    expect(clips).toHaveLength(1);
    expect(clips[0].tracks.map((track) => track.name)).toEqual(["Cube.scale"]);
  });
});
