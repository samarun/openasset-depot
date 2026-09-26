import { useEffect, useRef, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Eye,
  Pause,
  Play,
  Repeat2,
  ScanLine,
} from "lucide-react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { FBXLoader } from "three/examples/jsm/loaders/FBXLoader.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";

interface ThreeReviewPlayerProps {
  fileName: string;
  source: string;
  seekRequest?: { id: number; timecodeMs: number };
  onTimeChange: (timecodeMs: number, frame: number) => void;
}

interface Runtime {
  mixer?: THREE.AnimationMixer;
  clips: THREE.AnimationClip[];
  action?: THREE.AnimationAction;
  scene: THREE.Scene;
  model: THREE.Object3D;
  skeleton?: THREE.SkeletonHelper;
}

export function ThreeReviewPlayer({ fileName, source, seekRequest, onTimeChange }: ThreeReviewPlayerProps) {
  const mountRef = useRef<HTMLDivElement>(null);
  const runtimeRef = useRef<Runtime>();
  const playingRef = useRef(true);
  const onTimeChangeRef = useRef(onTimeChange);
  const lastReportedFrameRef = useRef(-1);
  const [playing, setPlaying] = useState(true);
  const [loop, setLoop] = useState(true);
  const [wireframe, setWireframe] = useState(false);
  const [showSkeleton, setShowSkeleton] = useState(false);
  const [animations, setAnimations] = useState<string[]>([]);
  const [activeAnimation, setActiveAnimation] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const [duration, setDuration] = useState(0);
  const [error, setError] = useState<string>();

  useEffect(() => {
    playingRef.current = playing;
  }, [playing]);

  useEffect(() => {
    onTimeChangeRef.current = onTimeChange;
  }, [onTimeChange]);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;
    let disposed = false;
    let frameId = 0;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x15191c);
    const camera = new THREE.PerspectiveCamera(42, 1, 0.01, 10_000);
    camera.position.set(3, 2, 5);
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    mount.appendChild(renderer.domElement);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    scene.add(new THREE.HemisphereLight(0xffffff, 0x29313a, 2.3));
    const keyLight = new THREE.DirectionalLight(0xffffff, 3.2);
    keyLight.position.set(4, 6, 5);
    scene.add(keyLight);
    const fillLight = new THREE.DirectionalLight(0x79d7c7, 1.2);
    fillLight.position.set(-4, 2, -3);
    scene.add(fillLight);
    const clock = new THREE.Clock();

    const resize = () => {
      const rect = mount.getBoundingClientRect();
      renderer.setSize(Math.max(1, rect.width), Math.max(1, rect.height), false);
      camera.aspect = Math.max(rect.width, 1) / Math.max(rect.height, 1);
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(mount);
    resize();

    const loaded = (model: THREE.Object3D, clips: THREE.AnimationClip[]) => {
      if (disposed) return;
      scene.add(model);
      frameModel(model, camera, controls);
      const runtime: Runtime = { clips, scene, model };
      if (clips.length > 0) {
        runtime.mixer = new THREE.AnimationMixer(model);
        runtime.action = runtime.mixer.clipAction(clips[0]);
        runtime.action.setLoop(THREE.LoopRepeat, Infinity).play();
        setDuration(clips[0].duration);
      }
      runtimeRef.current = runtime;
      setAnimations(clips.map((clip, index) => clip.name || `Animation ${index + 1}`));
      setError(undefined);
    };

    const extension = fileName.split(".").pop()?.toLowerCase();
    if (extension === "fbx") {
      new FBXLoader().load(source, (model) => loaded(model, model.animations), undefined, () => {
        if (!disposed) setError("This FBX could not be decoded. Export a GLB review proxy for the most reliable playback.");
      });
    } else {
      new GLTFLoader().load(source, (gltf) => loaded(gltf.scene, gltf.animations), undefined, () => {
        if (!disposed) setError("This model could not be decoded. Self-contained GLB files are recommended for review.");
      });
    }

    const animate = () => {
      if (disposed) return;
      frameId = requestAnimationFrame(animate);
      const delta = Math.min(clock.getDelta(), 0.1);
      const runtime = runtimeRef.current;
      if (runtime?.mixer && playingRef.current) runtime.mixer.update(delta);
      const current = runtime?.action?.time ?? runtime?.mixer?.time ?? 0;
      setElapsed(current);
      const currentFrame = Math.round(current * 24);
      if (currentFrame !== lastReportedFrameRef.current) {
        lastReportedFrameRef.current = currentFrame;
        onTimeChangeRef.current(Math.round(current * 1000), currentFrame);
      }
      controls.update();
      renderer.render(scene, camera);
    };
    animate();

    return () => {
      disposed = true;
      cancelAnimationFrame(frameId);
      observer.disconnect();
      controls.dispose();
      runtimeRef.current = undefined;
      scene.traverse((object) => disposeObject(object));
      renderer.dispose();
      renderer.domElement.remove();
    };
  }, [fileName, source]);

  useEffect(() => {
    const runtime = runtimeRef.current;
    const mixer = runtime?.mixer;
    if (!seekRequest || !runtime || !mixer) return;
    const seconds = Math.max(0, Math.min(seekRequest.timecodeMs / 1000, duration || Number.MAX_SAFE_INTEGER));
    if (runtime.action) {
      runtime.action.time = seconds;
      mixer.update(0);
    } else {
      mixer.setTime(seconds);
    }
    setElapsed(seconds);
    setPlaying(false);
  }, [duration, seekRequest]);

  function selectAnimation(index: number) {
    const runtime = runtimeRef.current;
    if (!runtime?.mixer || !runtime.clips[index]) return;
    runtime.action?.stop();
    runtime.action = runtime.mixer.clipAction(runtime.clips[index]);
    runtime.action.reset();
    runtime.action.setLoop(loop ? THREE.LoopRepeat : THREE.LoopOnce, loop ? Infinity : 1);
    runtime.action.clampWhenFinished = !loop;
    runtime.action.play();
    setActiveAnimation(index);
    setDuration(runtime.clips[index].duration);
    setElapsed(0);
  }

  function step(direction: -1 | 1) {
    const runtime = runtimeRef.current;
    if (!runtime?.mixer) return;
    const next = Math.max(0, Math.min((duration || Number.MAX_SAFE_INTEGER), elapsed + direction / 24));
    if (runtime.action) {
      runtime.action.time = next;
      runtime.mixer.update(0);
    } else {
      runtime.mixer.setTime(next);
    }
    setElapsed(next);
    setPlaying(false);
  }

  function toggleLoop() {
    const next = !loop;
    setLoop(next);
    const action = runtimeRef.current?.action;
    if (action) {
      action.setLoop(next ? THREE.LoopRepeat : THREE.LoopOnce, next ? Infinity : 1);
      action.clampWhenFinished = !next;
    }
  }

  function toggleWireframe() {
    const next = !wireframe;
    setWireframe(next);
    runtimeRef.current?.model.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      const materials = Array.isArray(object.material) ? object.material : [object.material];
      for (const material of materials) {
        if ("wireframe" in material) (material as THREE.MeshStandardMaterial).wireframe = next;
      }
    });
  }

  function toggleSkeleton() {
    const runtime = runtimeRef.current;
    if (!runtime) return;
    const next = !showSkeleton;
    setShowSkeleton(next);
    if (next) {
      runtime.skeleton = new THREE.SkeletonHelper(runtime.model);
      runtime.scene.add(runtime.skeleton);
    } else if (runtime.skeleton) {
      runtime.scene.remove(runtime.skeleton);
      runtime.skeleton.dispose();
      runtime.skeleton = undefined;
    }
  }

  return (
    <div className="three-review-player">
      <div ref={mountRef} className="three-review-viewport" />
      {error && <div className="review-media-error">{error}</div>}
      <div className="animation-toolbar" aria-label="Animation controls">
        <button type="button" onClick={() => step(-1)} aria-label="Previous keyframe"><ChevronLeft size={17} /></button>
        <button type="button" onClick={() => setPlaying((value) => !value)} aria-label={playing ? "Pause animation" : "Play animation"}>
          {playing ? <Pause size={17} /> : <Play size={17} />}
        </button>
        <button type="button" onClick={() => step(1)} aria-label="Next keyframe"><ChevronRight size={17} /></button>
        <span className="animation-time">{formatTime(elapsed)} / {formatTime(duration)} · F{Math.round(elapsed * 24)}</span>
        {animations.length > 0 && (
          <select value={activeAnimation} onChange={(event) => selectAnimation(Number(event.target.value))} aria-label="Animation clip">
            {animations.map((animation, index) => <option key={`${animation}-${index}`} value={index}>{animation}</option>)}
          </select>
        )}
        <button className={loop ? "is-active" : ""} type="button" onClick={toggleLoop} aria-label="Loop animation"><Repeat2 size={16} /></button>
        <button className={wireframe ? "is-active" : ""} type="button" onClick={toggleWireframe} aria-label="Toggle wireframe"><ScanLine size={16} /></button>
        <button className={showSkeleton ? "is-active" : ""} type="button" onClick={toggleSkeleton} aria-label="Toggle skeleton"><Eye size={16} /></button>
      </div>
    </div>
  );
}

function frameModel(model: THREE.Object3D, camera: THREE.PerspectiveCamera, controls: OrbitControls) {
  const box = new THREE.Box3().setFromObject(model);
  if (box.isEmpty()) return;
  const center = box.getCenter(new THREE.Vector3());
  const size = box.getSize(new THREE.Vector3());
  const radius = Math.max(size.x, size.y, size.z, 0.1);
  camera.near = Math.max(radius / 1_000, 0.001);
  camera.far = radius * 100;
  camera.position.copy(center).add(new THREE.Vector3(radius * 1.45, radius * 0.9, radius * 1.65));
  camera.updateProjectionMatrix();
  controls.target.copy(center);
  controls.update();
}

function disposeObject(object: THREE.Object3D) {
  if (!(object instanceof THREE.Mesh)) return;
  object.geometry?.dispose();
  const materials = Array.isArray(object.material) ? object.material : [object.material];
  for (const material of materials) material.dispose();
}

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds)) return "0:00.000";
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds - minutes * 60;
  return `${minutes}:${remainder.toFixed(3).padStart(6, "0")}`;
}
