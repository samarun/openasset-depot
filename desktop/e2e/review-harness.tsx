import React from "react";
import { createRoot } from "react-dom/client";
import { ThreeReviewPlayer } from "../src/components/ThreeReviewPlayer";

const query = new URLSearchParams(window.location.search);
const source = query.get("fixture") ?? "/e2e/fixtures/animated-cube.glb";
const frameRate = parseFrameRate(query.get("fps"));
const startFrame = Number(query.get("start") ?? "0");

function parseFrameRate(value: string | null): number | undefined {
  if (!value) return undefined;
  const [numerator, denominator = "1"] = value.split("/", 2);
  const result = Number(numerator) / Number(denominator);
  return Number.isFinite(result) && result > 0 ? result : undefined;
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ThreeReviewPlayer
      fileName={source}
      source={source}
      frameRate={frameRate}
      startFrame={Number.isInteger(startFrame) && startFrame >= 0 ? startFrame : 0}
      preserveDrawingBuffer
      onTimeChange={(timecodeMs, frame) => {
        document.body.dataset.timecodeMs = String(timecodeMs);
        if (frame != null) document.body.dataset.frame = String(frame);
      }}
    />
  </React.StrictMode>,
);
