// The 3D viewer in convert3d's preview pages. The page defines
// window.C3D_MODELS = [{ name, glb: "<base64>" }] before this runs.
// Rebuild dist/viewer.min.js with `npm install && npm run build` after editing.

import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";

const BACKGROUNDS = [0x1e1f22, 0x7f8287, 0xf2f2f2];

const $ = (id) => document.getElementById(id);
const viewport = $("viewport");

const renderer = new THREE.WebGLRenderer({ antialias: true });
renderer.setPixelRatio(window.devicePixelRatio);
renderer.toneMapping = THREE.NeutralToneMapping;
viewport.appendChild(renderer.domElement);

const scene = new THREE.Scene();
const pmrem = new THREE.PMREMGenerator(renderer);
scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
let bgIndex = 0;
scene.background = new THREE.Color(BACKGROUNDS[bgIndex]);

scene.environmentIntensity = 0.6;
const camera = new THREE.PerspectiveCamera(40, 1, 0.01, 1000);
// A key light that follows the camera keeps shapes readable from any side.
const key = new THREE.DirectionalLight(0xffffff, 1.2);
key.position.set(1, 2, 1.5);
key.target.position.set(0, 0, -1);
camera.add(key, key.target);
scene.add(camera);
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;

const content = new THREE.Group();
scene.add(content);
let grid = null;
let mixer = null;
let clips = [];
const clock = new THREE.Clock();

function resize() {
  const w = viewport.clientWidth;
  const h = viewport.clientHeight;
  renderer.setSize(w, h);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}
window.addEventListener("resize", resize);

function base64ToBuffer(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes.buffer;
}

function parseGlb(buffer) {
  return new Promise((resolve, reject) => new GLTFLoader().parse(buffer, "", resolve, reject));
}

function bounds() {
  const box = new THREE.Box3().setFromObject(content, true);
  return box.isEmpty() ? new THREE.Box3(new THREE.Vector3(-1, -1, -1), new THREE.Vector3(1, 1, 1)) : box;
}

function frame() {
  const box = bounds();
  const center = box.getCenter(new THREE.Vector3());
  const radius = Math.max(box.getSize(new THREE.Vector3()).length() / 2, 1e-4);
  const dist = radius / Math.sin(THREE.MathUtils.degToRad(camera.fov / 2)) * 1.1;
  const dir = new THREE.Vector3(1, 0.6, 1.4).normalize();
  camera.position.copy(center).addScaledVector(dir, dist);
  camera.near = dist / 100;
  camera.far = dist * 100;
  camera.updateProjectionMatrix();
  controls.target.copy(center);
  controls.update();
}

function rebuildGrid() {
  const visible = grid ? grid.visible : true;
  if (grid) scene.remove(grid);
  const box = bounds();
  const size = box.getSize(new THREE.Vector3());
  // Round the grid up to a power of ten so its cells mean something.
  const span = Math.max(size.x, size.z, 1e-3) * 3;
  const step = Math.pow(10, Math.floor(Math.log10(span / 2)));
  const extent = Math.ceil(span / step) * step;
  grid = new THREE.GridHelper(extent, Math.round((extent / step) * 2), 0x888888, 0x555555);
  grid.material.transparent = true;
  grid.material.opacity = 0.35;
  // Just under the model, so a floor in the model doesn't flicker against it.
  const below = box.min.y - size.length() * 0.002;
  grid.position.set((box.min.x + box.max.x) / 2, below, (box.min.z + box.max.z) / 2);
  grid.visible = visible;
  scene.add(grid);
}

function formatLength(m) {
  if (m >= 1) return m.toFixed(m >= 100 ? 0 : 2) + " m";
  if (m >= 0.01) return (m * 100).toFixed(1) + " cm";
  return (m * 1000).toFixed(1) + " mm";
}

function updateStats() {
  let meshes = 0;
  let tris = 0;
  let verts = 0;
  const materials = new Set();
  const textures = new Set();
  content.traverse((o) => {
    if (!o.isMesh) return;
    meshes++;
    const g = o.geometry;
    const count = g.attributes.position ? g.attributes.position.count : 0;
    verts += count;
    tris += (g.index ? g.index.count : count) / 3;
    for (const m of [].concat(o.material)) {
      materials.add(m);
      for (const v of Object.values(m)) if (v && v.isTexture) textures.add(v.source);
    }
  });
  const size = bounds().getSize(new THREE.Vector3());
  const rows = [
    ["Meshes", meshes.toLocaleString()],
    ["Triangles", Math.round(tris).toLocaleString()],
    ["Vertices", verts.toLocaleString()],
    ["Materials", materials.size.toLocaleString()],
    ["Textures", textures.size.toLocaleString()],
    ["Size", [size.x, size.y, size.z].map(formatLength).join(" × ")],
  ];
  if (clips.length) rows.push(["Animations", clips.length.toLocaleString()]);
  $("stats").innerHTML = rows.map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join("");
}

function setupAnimations() {
  const select = $("anim");
  select.innerHTML = "";
  mixer = clips.length ? new THREE.AnimationMixer(content) : null;
  $("anim-row").hidden = clips.length === 0;
  clips.forEach((c, i) => select.add(new Option(c.name || `Animation ${i + 1}`, i)));
  playClip(0);
}

function playClip(i) {
  if (!mixer) return;
  mixer.stopAllAction();
  mixer.clipAction(clips[i]).play();
}

async function load(models, title) {
  content.clear();
  clips = [];
  const failures = [];
  for (const m of models) {
    try {
      const gltf = await parseGlb(m.buffer);
      gltf.scene.name = m.name;
      content.add(gltf.scene);
      clips.push(...gltf.animations);
    } catch (e) {
      failures.push(`${m.name}: ${e.message || e}`);
    }
  }
  $("title").textContent = title;
  document.title = `${title} – Preview 3D`;
  $("error").hidden = failures.length === 0;
  $("error").textContent = failures.join("\n");
  setupAnimations();
  rebuildGrid();
  frame();
  updateStats();
  setWireframe(wireframe);
}

let wireframe = false;
function setWireframe(on) {
  wireframe = on;
  content.traverse((o) => {
    if (o.isMesh) for (const m of [].concat(o.material)) m.wireframe = on;
  });
  $("wire").classList.toggle("on", on);
}

function toggleGrid() {
  grid.visible = !grid.visible;
  $("grid").classList.toggle("on", grid.visible);
}

function cycleBackground() {
  bgIndex = (bgIndex + 1) % BACKGROUNDS.length;
  scene.background.setHex(BACKGROUNDS[bgIndex]);
  document.body.classList.toggle("light", bgIndex === BACKGROUNDS.length - 1);
}

$("wire").onclick = () => setWireframe(!wireframe);
$("grid").onclick = toggleGrid;
$("bg").onclick = cycleBackground;
$("frame").onclick = frame;
$("anim").onchange = (e) => playClip(Number(e.target.value));
window.addEventListener("keydown", (e) => {
  if (e.target.tagName === "SELECT" || e.ctrlKey || e.metaKey || e.altKey) return;
  const actions = { f: frame, w: () => setWireframe(!wireframe), g: toggleGrid, b: cycleBackground };
  actions[e.key.toLowerCase()]?.();
});

// Dropping .glb files onto the page views them instead.
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", async (e) => {
  e.preventDefault();
  const files = [...e.dataTransfer.files].filter((f) => /\.glb$/i.test(f.name));
  if (!files.length) return;
  const models = await Promise.all(files.map(async (f) => ({ name: f.name, buffer: await f.arrayBuffer() })));
  load(models, files.map((f) => f.name).join(", "));
});

renderer.setAnimationLoop(() => {
  if (mixer) mixer.update(clock.getDelta());
  controls.update();
  renderer.render(scene, camera);
});

resize();
load(
  window.C3D_MODELS.map((m) => ({ name: m.name, buffer: base64ToBuffer(m.glb) })),
  window.C3D_TITLE || "Preview 3D",
);
