import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  ArrowUpRight,
  Check,
  CheckCircle2,
  Clock3,
  Eraser,
  Highlighter,
  MessageSquare,
  PenLine,
  RectangleHorizontal,
  RotateCcw,
  Send,
  X,
} from "lucide-react";
import type {
  AnnotationTool,
  AssetFile,
  ReviewAnnotation,
  ReviewComment,
  ReviewMedia,
} from "../types/domain";
import { AnnotationCanvas } from "./AnnotationCanvas";

const ThreeReviewPlayer = lazy(() => import("./ThreeReviewPlayer").then((module) => ({
  default: module.ThreeReviewPlayer,
})));

export interface ReviewCommentDraft {
  body: string;
  timecodeMs?: number;
  frameNumber?: number;
  annotation?: { marks: ReviewAnnotation[] };
}

interface ReviewViewerProps {
  file?: AssetFile;
  open: boolean;
  onClose: () => void;
  loadMedia: (file: AssetFile) => Promise<ReviewMedia | undefined>;
  loadComments: (file: AssetFile) => Promise<ReviewComment[]>;
  createComment: (file: AssetFile, draft: ReviewCommentDraft) => Promise<ReviewComment>;
  resolveComment: (comment: ReviewComment, resolved: boolean) => Promise<ReviewComment>;
}

const REVIEW_FPS = 24;

export function ReviewViewer({
  file,
  open,
  onClose,
  loadMedia,
  loadComments,
  createComment,
  resolveComment,
}: ReviewViewerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const [media, setMedia] = useState<ReviewMedia>();
  const [mediaUrl, setMediaUrl] = useState<string>();
  const [comments, setComments] = useState<ReviewComment[]>([]);
  const [selectedCommentId, setSelectedCommentId] = useState<string>();
  const [commentBody, setCommentBody] = useState("");
  const [draftMarks, setDraftMarks] = useState<ReviewAnnotation[]>([]);
  const [activeTool, setActiveTool] = useState<AnnotationTool>();
  const [annotationColor, setAnnotationColor] = useState("#ffcb4c");
  const [annotationWidth, setAnnotationWidth] = useState(4);
  const [annotationsVisible, setAnnotationsVisible] = useState(true);
  const [timecodeMs, setTimecodeMs] = useState(0);
  const [frameNumber, setFrameNumber] = useState(0);
  const [seekRequest, setSeekRequest] = useState<{ id: number; timecodeMs: number }>();
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string>();

  useEffect(() => {
    if (!open || !file) return;
    let active = true;
    let objectUrl: string | undefined;
    setLoading(true);
    setError(undefined);
    setMedia(undefined);
    setMediaUrl(undefined);
    setComments([]);
    setSelectedCommentId(undefined);
    setDraftMarks([]);
    setCommentBody("");
    void Promise.all([loadMedia(file), loadComments(file)])
      .then(([nextMedia, nextComments]) => {
        if (!active) return;
        if (nextMedia) {
          objectUrl = URL.createObjectURL(nextMedia.blob);
          setMedia(nextMedia);
          setMediaUrl(objectUrl);
        }
        setComments(nextComments);
      })
      .catch((loadError) => {
        if (active) setError(loadError instanceof Error ? loadError.message : "Review could not be loaded.");
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
      if (objectUrl) {
        const staleObjectUrl = objectUrl;
        // Three.js may still be unwinding its loader when the review closes.
        window.setTimeout(() => URL.revokeObjectURL(staleObjectUrl), 1_000);
      }
    };
  }, [file, loadComments, loadMedia, open]);

  const visibleMarks = useMemo(() => {
    if (!annotationsVisible) return draftMarks;
    const selected = comments.find((comment) => comment.id === selectedCommentId);
    if (selected?.annotation?.marks) return [...selected.annotation.marks, ...draftMarks];
    const contextual = comments
      .filter((comment) => !comment.resolved_at)
      .filter((comment) => comment.timecode_ms == null || Math.abs(comment.timecode_ms - timecodeMs) <= 750)
      .flatMap((comment) => comment.annotation?.marks ?? []);
    return [...contextual, ...draftMarks];
  }, [annotationsVisible, comments, draftMarks, selectedCommentId, timecodeMs]);

  if (!open || !file) return null;
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  const timedMedia = isTimedMedia(extension) || isTimedContentType(media?.contentType);

  async function submitComment(event: FormEvent) {
    event.preventDefault();
    const body = commentBody.trim();
    if (!body) return;
    setSubmitting(true);
    setError(undefined);
    try {
      const created = await createComment(file!, {
        body,
        timecodeMs: timedMedia ? timecodeMs : undefined,
        frameNumber: timedMedia ? frameNumber : undefined,
        annotation: draftMarks.length > 0 ? { marks: draftMarks } : undefined,
      });
      setComments((current) => [...current, created]);
      setSelectedCommentId(created.id);
      setCommentBody("");
      setDraftMarks([]);
      setActiveTool(undefined);
    } catch (submitError) {
      setError(submitError instanceof Error ? submitError.message : "Comment could not be posted.");
    } finally {
      setSubmitting(false);
    }
  }

  async function toggleResolved(comment: ReviewComment) {
    try {
      const updated = await resolveComment(comment, !comment.resolved_at);
      setComments((current) => current.map((item) => item.id === updated.id ? updated : item));
    } catch (resolveError) {
      setError(resolveError instanceof Error ? resolveError.message : "Comment could not be updated.");
    }
  }

  function selectComment(comment: ReviewComment) {
    setSelectedCommentId(comment.id);
    if (comment.timecode_ms == null) return;
    setTimecodeMs(comment.timecode_ms);
    setFrameNumber(comment.frame_number ?? Math.round(comment.timecode_ms / 1000 * REVIEW_FPS));
    if (videoRef.current) videoRef.current.currentTime = comment.timecode_ms / 1000;
    if (audioRef.current) audioRef.current.currentTime = comment.timecode_ms / 1000;
    setSeekRequest({ id: Date.now(), timecodeMs: comment.timecode_ms });
  }

  function updateMediaTime(seconds: number) {
    const milliseconds = Math.round(seconds * 1000);
    setTimecodeMs(milliseconds);
    setFrameNumber(Math.round(seconds * REVIEW_FPS));
  }

  return (
    <div className="review-overlay" role="dialog" aria-modal="true" aria-label={`Review ${file.name}`}>
      <section className="review-shell">
        <header className="review-header">
          <div>
            <span className="eyebrow">Revision review</span>
            <h2>{file.name}</h2>
            <p>{file.path} · Version {file.revision}</p>
          </div>
          <div className="review-header-actions">
            <span className="review-format-pill">{extension.toUpperCase() || "FILE"}</span>
            <button className="icon-button" type="button" onClick={onClose} aria-label="Close review"><X size={19} /></button>
          </div>
        </header>

        <div className="review-layout">
          <section className="review-stage-column">
            <div className={`review-media-stage ${activeTool ? "is-annotating" : ""}`}>
              {loading && <div className="review-media-empty">Preparing review media…</div>}
              {!loading && mediaUrl && renderMedia({
                extension,
                file,
                media,
                mediaUrl,
                videoRef,
                audioRef,
                seekRequest,
                onTimeChange: updateMediaTime,
              })}
              {!loading && !mediaUrl && (
                <div className="review-media-empty">
                  <MessageSquare size={28} />
                  <strong>No review proxy yet</strong>
                  <span>Submit a rendered preview from Blender, Maya, Houdini, Unreal, or another DCC integration.</span>
                </div>
              )}
              <AnnotationCanvas
                annotations={visibleMarks}
                tool={activeTool}
                color={annotationColor}
                width={activeTool === "highlighter" ? Math.max(10, annotationWidth * 3) : annotationWidth}
                onComplete={(annotation) => setDraftMarks((current) => [...current, annotation])}
              />
            </div>

            <div className="annotation-toolbar" aria-label="Annotation tools">
              <ToolButton label="Pen" active={activeTool === "pen"} onClick={() => setActiveTool(activeTool === "pen" ? undefined : "pen")}><PenLine size={17} /></ToolButton>
              <ToolButton label="Highlighter" active={activeTool === "highlighter"} onClick={() => setActiveTool(activeTool === "highlighter" ? undefined : "highlighter")}><Highlighter size={17} /></ToolButton>
              <ToolButton label="Rectangle" active={activeTool === "rectangle"} onClick={() => setActiveTool(activeTool === "rectangle" ? undefined : "rectangle")}><RectangleHorizontal size={17} /></ToolButton>
              <ToolButton label="Arrow" active={activeTool === "arrow"} onClick={() => setActiveTool(activeTool === "arrow" ? undefined : "arrow")}><ArrowUpRight size={17} /></ToolButton>
              <label className="annotation-color" title="Annotation color">
                <input type="color" value={annotationColor} onChange={(event) => setAnnotationColor(event.target.value)} />
              </label>
              <label className="annotation-size">
                <span>Size</span>
                <input type="range" min="2" max="12" value={annotationWidth} onChange={(event) => setAnnotationWidth(Number(event.target.value))} />
              </label>
              <button type="button" className={annotationsVisible ? "is-active" : ""} onClick={() => setAnnotationsVisible((value) => !value)}>
                {annotationsVisible ? <Check size={16} /> : <X size={16} />} Sketches
              </button>
              <button type="button" onClick={() => setDraftMarks((current) => current.slice(0, -1))} disabled={draftMarks.length === 0}>
                <RotateCcw size={16} /> Undo
              </button>
              <button type="button" onClick={() => setDraftMarks([])} disabled={draftMarks.length === 0}>
                <Eraser size={16} /> Clear
              </button>
            </div>
          </section>

          <aside className="review-comments-panel">
            <header>
              <div>
                <span className="eyebrow">Feedback</span>
                <h3>Comments</h3>
              </div>
              <span>{comments.filter((comment) => !comment.resolved_at).length} open</span>
            </header>

            <div className="review-comments-list">
              {comments.length === 0 && !loading && (
                <div className="review-comments-empty">
                  <MessageSquare size={22} />
                  <strong>Start the conversation</strong>
                  <span>Pause on a frame, draw a note, and post precise feedback.</span>
                </div>
              )}
              {comments.map((comment) => (
                <article
                  className={`review-comment ${selectedCommentId === comment.id ? "is-selected" : ""} ${comment.resolved_at ? "is-resolved" : ""}`}
                  key={comment.id}
                >
                  <header>
                    <strong>{comment.author}</strong>
                    <time>{formatRelativeDate(comment.created_at)}</time>
                  </header>
                  <p>{comment.body}</p>
                  <footer>
                    {comment.timecode_ms != null && (
                      <button type="button" onClick={() => selectComment(comment)}>
                        <Clock3 size={13} /> {formatTimecode(comment.timecode_ms)} · F{comment.frame_number ?? 0}
                      </button>
                    )}
                    {comment.annotation?.marks?.length ? (
                      <button type="button" onClick={() => selectComment(comment)}><PenLine size={13} /> Show sketch</button>
                    ) : null}
                    <button type="button" onClick={() => void toggleResolved(comment)}>
                      <CheckCircle2 size={13} /> {comment.resolved_at ? "Reopen" : "Resolve"}
                    </button>
                  </footer>
                </article>
              ))}
            </div>

            <form className="review-comment-form" onSubmit={(event) => void submitComment(event)}>
              {timedMedia && (
                <span className="review-current-time"><Clock3 size={14} /> {formatTimecode(timecodeMs)} · Frame {frameNumber}</span>
              )}
              {draftMarks.length > 0 && <span className="review-sketch-count"><PenLine size={14} /> {draftMarks.length} sketch mark{draftMarks.length === 1 ? "" : "s"}</span>}
              <textarea
                value={commentBody}
                onChange={(event) => setCommentBody(event.target.value)}
                placeholder="Leave clear, actionable feedback…"
                maxLength={4000}
                rows={4}
              />
              {error && <p className="form-error">{error}</p>}
              <button className="primary-button" type="submit" disabled={submitting || !commentBody.trim()}>
                <Send size={16} /> {submitting ? "Posting…" : "Post Comment"}
              </button>
            </form>
          </aside>
        </div>
      </section>
    </div>
  );
}

interface RenderMediaArgs {
  extension: string;
  file: AssetFile;
  media?: ReviewMedia;
  mediaUrl: string;
  videoRef: React.RefObject<HTMLVideoElement>;
  audioRef: React.RefObject<HTMLAudioElement>;
  seekRequest?: { id: number; timecodeMs: number };
  onTimeChange: (seconds: number) => void;
}

function renderMedia({ extension, file, media, mediaUrl, videoRef, audioRef, seekRequest, onTimeChange }: RenderMediaArgs) {
  const contentType = media?.contentType ?? "";
  if ((isModel(extension) || isModelContentType(contentType)) && media?.source === "asset") {
    const playerName = reviewPlayerFileName(file.name, contentType);
    return (
      <Suspense fallback={<div className="review-media-empty">Loading 3D review engine…</div>}>
        <ThreeReviewPlayer
          fileName={playerName}
          source={mediaUrl}
          seekRequest={seekRequest}
          onTimeChange={(milliseconds) => onTimeChange(milliseconds / 1000)}
        />
      </Suspense>
    );
  }
  if ((isVideo(extension) || contentType.startsWith("video/")) && media?.source === "asset") {
    return <video ref={videoRef} className="review-video" src={mediaUrl} controls loop onTimeUpdate={(event) => onTimeChange(event.currentTarget.currentTime)} />;
  }
  if ((isAudio(extension) || contentType.startsWith("audio/")) && media?.source === "asset") {
    return (
      <div className="review-audio-wrap">
        <div className="audio-waveform" aria-hidden="true">{Array.from({ length: 72 }, (_, index) => <i key={index} style={{ height: `${18 + ((index * 37) % 64)}%` }} />)}</div>
        <audio ref={audioRef} src={mediaUrl} controls onTimeUpdate={(event) => onTimeChange(event.currentTarget.currentTime)} />
      </div>
    );
  }
  return <img className="review-image" src={mediaUrl} alt={`${file.name} review`} />;
}

export function reviewPlayerFileName(originalName: string, contentType: string): string {
  const normalizedType = contentType.split(";", 1)[0].trim().toLowerCase();
  if (normalizedType === "application/vnd.autodesk.fbx") return "review.fbx";
  if (normalizedType === "model/gltf+json") return "review.gltf";
  if (normalizedType === "model/gltf-binary") return "review.glb";
  return originalName;
}

function ToolButton({ label, active, onClick, children }: { label: string; active: boolean; onClick: () => void; children: React.ReactNode }) {
  return <button className={active ? "is-active" : ""} type="button" onClick={onClick} aria-label={label}>{children}<span>{label}</span></button>;
}

function isModel(extension: string): boolean {
  return ["fbx", "glb", "gltf"].includes(extension);
}

function isVideo(extension: string): boolean {
  return ["mp4", "m4v", "webm", "mov"].includes(extension);
}

function isAudio(extension: string): boolean {
  return ["mp3", "wav", "ogg", "oga"].includes(extension);
}

function isTimedMedia(extension: string): boolean {
  return isModel(extension) || isVideo(extension) || isAudio(extension);
}

function isModelContentType(contentType: string): boolean {
  return contentType.startsWith("model/") || contentType.includes("autodesk.fbx");
}

function isTimedContentType(contentType?: string): boolean {
  if (!contentType) return false;
  return isModelContentType(contentType) || contentType.startsWith("video/") || contentType.startsWith("audio/");
}

function formatTimecode(milliseconds: number): string {
  const totalSeconds = Math.max(0, milliseconds) / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds - minutes * 60;
  return `${minutes}:${seconds.toFixed(3).padStart(6, "0")}`;
}

function formatRelativeDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "just now";
  return date.toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}
