import { useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import type { AnnotationPoint, AnnotationTool, ReviewAnnotation } from "../types/domain";

interface AnnotationCanvasProps {
  annotations: ReviewAnnotation[];
  tool?: AnnotationTool;
  color: string;
  width: number;
  onComplete: (annotation: ReviewAnnotation) => void;
}

export function AnnotationCanvas({ annotations, tool, color, width, onComplete }: AnnotationCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const draftRef = useRef<ReviewAnnotation>();
  const [draft, setDraft] = useState<ReviewAnnotation>();

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const redraw = () => drawAnnotations(canvas, [...annotations, ...(draft ? [draft] : [])]);
    redraw();
    const observer = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(redraw);
    observer?.observe(canvas);
    return () => observer?.disconnect();
  }, [annotations, draft]);

  function pointerDown(event: ReactPointerEvent<HTMLCanvasElement>) {
    if (!tool) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const point = normalizedPoint(event);
    const next = { tool, color, width, points: [point] } satisfies ReviewAnnotation;
    draftRef.current = next;
    setDraft(next);
  }

  function pointerMove(event: ReactPointerEvent<HTMLCanvasElement>) {
    const current = draftRef.current;
    if (!current) return;
    const point = normalizedPoint(event);
    const next = current.tool === "rectangle" || current.tool === "arrow"
      ? { ...current, points: [current.points[0], point] }
      : { ...current, points: [...current.points, point] };
    draftRef.current = next;
    setDraft(next);
  }

  function pointerUp(event: ReactPointerEvent<HTMLCanvasElement>) {
    const current = draftRef.current;
    if (!current) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (current.points.length > 1) onComplete(current);
    draftRef.current = undefined;
    setDraft(undefined);
  }

  return (
    <canvas
      ref={canvasRef}
      className={`annotation-canvas ${tool ? "is-drawing" : ""}`}
      aria-label={tool ? `Draw ${tool} annotation` : "Review annotations"}
      onPointerDown={pointerDown}
      onPointerMove={pointerMove}
      onPointerUp={pointerUp}
      onPointerCancel={() => {
        draftRef.current = undefined;
        setDraft(undefined);
      }}
    />
  );
}

function normalizedPoint(event: ReactPointerEvent<HTMLCanvasElement>): AnnotationPoint {
  const rect = event.currentTarget.getBoundingClientRect();
  return {
    x: clamp((event.clientX - rect.left) / Math.max(rect.width, 1)),
    y: clamp((event.clientY - rect.top) / Math.max(rect.height, 1)),
  };
}

function clamp(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function drawAnnotations(canvas: HTMLCanvasElement, annotations: ReviewAnnotation[]) {
  const rect = canvas.getBoundingClientRect();
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, Math.round(rect.width));
  const height = Math.max(1, Math.round(rect.height));
  const pixelWidth = Math.round(width * ratio);
  const pixelHeight = Math.round(height * ratio);
  if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
    canvas.width = pixelWidth;
    canvas.height = pixelHeight;
  }
  const context = canvas.getContext("2d");
  if (!context) return;
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, width, height);
  for (const annotation of annotations) {
    drawAnnotation(context, annotation, width, height);
  }
}

function drawAnnotation(
  context: CanvasRenderingContext2D,
  annotation: ReviewAnnotation,
  width: number,
  height: number,
) {
  const points = annotation.points.map((point) => ({ x: point.x * width, y: point.y * height }));
  if (points.length < 2) return;
  context.save();
  context.strokeStyle = annotation.color;
  context.fillStyle = annotation.color;
  context.lineWidth = annotation.width;
  context.lineCap = "round";
  context.lineJoin = "round";
  context.globalAlpha = annotation.tool === "highlighter" ? 0.38 : 0.96;

  const start = points[0];
  const end = points[points.length - 1];
  if (annotation.tool === "rectangle") {
    context.strokeRect(start.x, start.y, end.x - start.x, end.y - start.y);
  } else if (annotation.tool === "arrow") {
    const angle = Math.atan2(end.y - start.y, end.x - start.x);
    const head = Math.max(10, annotation.width * 3);
    context.beginPath();
    context.moveTo(start.x, start.y);
    context.lineTo(end.x, end.y);
    context.stroke();
    context.beginPath();
    context.moveTo(end.x, end.y);
    context.lineTo(end.x - head * Math.cos(angle - Math.PI / 6), end.y - head * Math.sin(angle - Math.PI / 6));
    context.lineTo(end.x - head * Math.cos(angle + Math.PI / 6), end.y - head * Math.sin(angle + Math.PI / 6));
    context.closePath();
    context.fill();
  } else {
    context.beginPath();
    context.moveTo(start.x, start.y);
    for (const point of points.slice(1)) context.lineTo(point.x, point.y);
    context.stroke();
  }
  context.restore();
}
