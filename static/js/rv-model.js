/**
 * Fossall proposed 40′ container-scale EV-RV — interactive Three.js sketch.
 * Form factor inspired by cabless autonomous haulers (low skateboard chassis,
 * sensor pods, container lock interface) adapted for a living module.
 *
 * Drag to orbit · scroll to zoom · right-drag to pan.
 */

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

// ISO 40′ high-cube exterior (metres). Scene unit = 1 m.
const L = 12.192;
const W = 2.438;
const H = 2.896;
const CHASSIS_H = 0.55;
const GROUND_CLEAR = 0.32;
const NOSE_LEN = 1.1;

const COLORS = {
  chassis: 0x2a2e2c,
  chassisAccent: 0x3d8f7a,
  body: 0x3a4240,
  bodyLight: 0x4a5552,
  corrugation: 0x323836,
  glass: 0x7ec8b8,
  glassTint: 0xa8ddd0,
  metal: 0xc5c9c6,
  darkMetal: 0x1a1c1b,
  tire: 0x1c1c1c,
  rim: 0xe8ebe8,
  led: 0xf0fff8,
  sensor: 0xc45c4a,
  sensorLens: 0x1a3040,
  interior: 0xe8e0d4,
  floor: 0x8b7355,
  furniture: 0xc4b8a5,
  wet: 0xd0d8d6,
  solar: 0x1a2830,
  ground: 0xd4cfc4,
  groundDark: 0xc0b8a8,
};

/**
 * @param {HTMLElement} host
 */
export function mountRvModel(host) {
  const canvas = host.querySelector("canvas");
  const status = host.querySelector("[data-rv-status]");
  const btnReset = host.querySelector('[data-rv-action="reset"]');
  const btnRotate = host.querySelector('[data-rv-action="rotate"]');
  const btnCutaway = host.querySelector('[data-rv-action="cutaway"]');
  const btnChassis = host.querySelector('[data-rv-action="chassis"]');
  const btnLabels = host.querySelector('[data-rv-action="labels"]');

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0xf0ebe3);
  scene.fog = new THREE.Fog(0xf0ebe3, 28, 55);

  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: true,
    alpha: false,
    powerPreference: "high-performance",
  });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;

  const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 120);
  camera.position.set(11, 5.5, 10);

  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = true;
  controls.dampingFactor = 0.06;
  controls.minDistance = 4;
  controls.maxDistance = 35;
  controls.maxPolarAngle = Math.PI * 0.49;
  controls.target.set(0, 1.4, 0);
  controls.autoRotate = true;
  controls.autoRotateSpeed = 0.55;

  // Lights
  scene.add(new THREE.AmbientLight(0xfff8f0, 0.55));
  const hemi = new THREE.HemisphereLight(0xf5f0e8, 0x6a7568, 0.55);
  scene.add(hemi);
  const sun = new THREE.DirectionalLight(0xfff5e8, 1.15);
  sun.position.set(8, 16, 6);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.camera.near = 1;
  sun.shadow.camera.far = 40;
  sun.shadow.camera.left = -14;
  sun.shadow.camera.right = 14;
  sun.shadow.camera.top = 10;
  sun.shadow.camera.bottom = -10;
  sun.shadow.bias = -0.0002;
  scene.add(sun);
  const fill = new THREE.DirectionalLight(0xc8e0f0, 0.35);
  fill.position.set(-6, 4, -8);
  scene.add(fill);
  const rim = new THREE.DirectionalLight(0x5dba9a, 0.25);
  rim.position.set(0, 3, -12);
  scene.add(rim);

  // Ground
  const ground = new THREE.Mesh(
    new THREE.CircleGeometry(22, 64),
    new THREE.MeshStandardMaterial({
      color: COLORS.ground,
      roughness: 0.95,
      metalness: 0.02,
    }),
  );
  ground.rotation.x = -Math.PI / 2;
  ground.receiveShadow = true;
  scene.add(ground);

  const ring = new THREE.Mesh(
    new THREE.RingGeometry(8.5, 8.65, 96),
    new THREE.MeshBasicMaterial({
      color: COLORS.chassisAccent,
      transparent: true,
      opacity: 0.22,
      side: THREE.DoubleSide,
    }),
  );
  ring.rotation.x = -Math.PI / 2;
  ring.position.y = 0.01;
  scene.add(ring);

  // Vehicle root
  const vehicle = new THREE.Group();
  vehicle.name = "vehicle";
  scene.add(vehicle);

  const chassisGroup = new THREE.Group();
  chassisGroup.name = "chassis";
  vehicle.add(chassisGroup);

  const bodyGroup = new THREE.Group();
  bodyGroup.name = "body";
  vehicle.add(bodyGroup);

  const interiorGroup = new THREE.Group();
  interiorGroup.name = "interior";
  interiorGroup.visible = false;
  vehicle.add(interiorGroup);

  const labelsGroup = new THREE.Group();
  labelsGroup.name = "labels";
  labelsGroup.visible = true;
  scene.add(labelsGroup);

  buildChassis(chassisGroup);
  buildBody(bodyGroup);
  buildInterior(interiorGroup);
  buildDimensionLabels(labelsGroup);

  // Center vehicle on ground (wheels touch y=0)
  vehicle.position.y = 0;

  // State
  let cutaway = false;
  let chassisOnly = false;
  let disposed = false;

  function applyVisibility() {
    bodyGroup.visible = !chassisOnly;
    interiorGroup.visible = cutaway && !chassisOnly;
    // In cutaway, hide near-side wall panels (already handled via named meshes)
    bodyGroup.traverse((obj) => {
      if (obj.userData.cutawayHide) {
        obj.visible = !cutaway;
      }
      if (obj.userData.cutawayOnly) {
        obj.visible = cutaway;
      }
    });
  }

  function resize() {
    const rect = host.getBoundingClientRect();
    const w = Math.max(1, Math.floor(rect.width));
    const h = Math.max(1, Math.floor(rect.height));
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h, false);
  }

  const ro = new ResizeObserver(resize);
  ro.observe(host);
  resize();

  function animate() {
    if (disposed) return;
    requestAnimationFrame(animate);
    controls.update();
    renderer.render(scene, camera);
  }
  animate();

  // UI
  function setActive(btn, on) {
    if (!btn) return;
    btn.classList.toggle("is-active", on);
    btn.setAttribute("aria-pressed", on ? "true" : "false");
  }

  btnReset?.addEventListener("click", () => {
    camera.position.set(11, 5.5, 10);
    controls.target.set(0, 1.4, 0);
    controls.update();
  });

  btnRotate?.addEventListener("click", () => {
    controls.autoRotate = !controls.autoRotate;
    setActive(btnRotate, controls.autoRotate);
  });
  setActive(btnRotate, true);

  btnCutaway?.addEventListener("click", () => {
    cutaway = !cutaway;
    if (cutaway) chassisOnly = false;
    setActive(btnCutaway, cutaway);
    setActive(btnChassis, chassisOnly);
    applyVisibility();
  });

  btnChassis?.addEventListener("click", () => {
    chassisOnly = !chassisOnly;
    if (chassisOnly) cutaway = false;
    setActive(btnChassis, chassisOnly);
    setActive(btnCutaway, cutaway);
    applyVisibility();
  });

  btnLabels?.addEventListener("click", () => {
    labelsGroup.visible = !labelsGroup.visible;
    setActive(btnLabels, labelsGroup.visible);
  });
  setActive(btnLabels, true);

  applyVisibility();

  if (status) {
    status.textContent =
      "Drag to orbit · scroll zoom · right-drag pan · 40′ high-cube envelope";
  }

  // Prefer-reduced-motion
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    controls.autoRotate = false;
    setActive(btnRotate, false);
  }

  // Cleanup when host is removed (HTMX swap)
  const observer = new MutationObserver(() => {
    if (!document.body.contains(host)) {
      disposed = true;
      ro.disconnect();
      observer.disconnect();
      controls.dispose();
      renderer.dispose();
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });

  return { dispose: () => { disposed = true; } };
}

// ── Builders ──────────────────────────────────────────────────────────────

function mat(color, opts = {}) {
  return new THREE.MeshStandardMaterial({
    color,
    roughness: opts.roughness ?? 0.55,
    metalness: opts.metalness ?? 0.15,
    transparent: opts.transparent ?? false,
    opacity: opts.opacity ?? 1,
    side: opts.side ?? THREE.FrontSide,
    emissive: opts.emissive ?? 0x000000,
    emissiveIntensity: opts.emissiveIntensity ?? 0,
  });
}

function box(w, h, d, material, x, y, z, parent, extras = {}) {
  const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), material);
  mesh.position.set(x, y, z);
  mesh.castShadow = extras.cast !== false;
  mesh.receiveShadow = extras.receive !== false;
  if (extras.userData) Object.assign(mesh.userData, extras.userData);
  parent.add(mesh);
  return mesh;
}

function buildChassis(group) {
  const totalLen = L + NOSE_LEN + 0.35;
  const y0 = GROUND_CLEAR + CHASSIS_H / 2;

  // Main skateboard deck
  const deckMat = mat(COLORS.chassis, { roughness: 0.45, metalness: 0.35 });
  box(totalLen, CHASSIS_H, W * 0.96, deckMat, NOSE_LEN / 2 - 0.1, y0, 0, group);

  // Lower battery pack bulge
  const batMat = mat(COLORS.darkMetal, { roughness: 0.4, metalness: 0.5 });
  box(
    L * 0.72,
    0.22,
    W * 0.72,
    batMat,
    0,
    GROUND_CLEAR + 0.12,
    0,
    group,
  );

  // Teal accent bands (Humble-inspired)
  const accent = mat(COLORS.chassisAccent, {
    roughness: 0.4,
    metalness: 0.2,
    emissive: COLORS.chassisAccent,
    emissiveIntensity: 0.08,
  });
  const bandY = y0;
  for (const z of [W * 0.48, -W * 0.48]) {
    box(0.12, CHASSIS_H * 0.92, 0.04, accent, -L * 0.22, bandY, z, group);
    box(0.12, CHASSIS_H * 0.92, 0.04, accent, L * 0.22, bandY, z, group);
  }

  // Nose fairing (cabless autonomy nose)
  buildNose(group, y0);

  // Rear bumper bar
  box(
    0.18,
    CHASSIS_H * 0.7,
    W * 0.9,
    mat(COLORS.darkMetal, { metalness: 0.4 }),
    -L / 2 - 0.2,
    y0 - 0.05,
    0,
    group,
  );

  // LED strips rear
  const ledMat = mat(COLORS.led, {
    roughness: 0.3,
    metalness: 0.1,
    emissive: 0xff3344,
    emissiveIntensity: 0.7,
  });
  for (const z of [-0.7, 0.7]) {
    box(0.06, 0.08, 0.35, ledMat, -L / 2 - 0.28, y0 + 0.05, z, group);
  }

  // Wheels — dual tandem front + dual tandem rear (Humble-like multi-axle)
  const wheelX = [
    L / 2 - 1.2,
    L / 2 - 2.35,
    -L / 2 + 2.35,
    -L / 2 + 1.2,
  ];
  for (const x of wheelX) {
    addWheelPair(group, x);
  }

  // Sensor pods at chassis corners
  const podY = y0 + CHASSIS_H / 2 + 0.08;
  const podXs = [L / 2 + NOSE_LEN * 0.15, -L / 2 - 0.05];
  const podZs = [W * 0.42, -W * 0.42];
  for (const x of podXs) {
    for (const z of podZs) {
      addSensorPod(group, x, podY, z);
    }
  }

  // Twist-lock indicators under container corners
  const lockMat = mat(COLORS.metal, { metalness: 0.7, roughness: 0.3 });
  for (const x of [-L / 2 + 0.15, L / 2 - 0.15]) {
    for (const z of [-W / 2 + 0.12, W / 2 - 0.12]) {
      box(0.18, 0.08, 0.18, lockMat, x, y0 + CHASSIS_H / 2 + 0.04, z, group);
    }
  }
}

function buildNose(group, y0) {
  // Low chassis nose (under body) + taller cabless aero fairing (Humble-like).
  const noseMat = mat(COLORS.chassis, { roughness: 0.4, metalness: 0.3 });
  const fairingMat = mat(COLORS.body, { roughness: 0.42, metalness: 0.18 });

  const nose = new THREE.Mesh(
    new THREE.BoxGeometry(NOSE_LEN, CHASSIS_H * 1.05, W * 0.92),
    noseMat,
  );
  nose.position.set(L / 2 + NOSE_LEN / 2 - 0.15, y0, 0);
  nose.castShadow = true;
  group.add(nose);

  // Full-height rounded fairing ahead of the container (cabless “face”)
  const deckTop = GROUND_CLEAR + CHASSIS_H + 0.06;
  const fairingH = H * 0.92;
  const fairing = new THREE.Mesh(
    new THREE.BoxGeometry(NOSE_LEN * 0.95, fairingH, W * 0.9),
    fairingMat,
  );
  fairing.position.set(
    L / 2 + NOSE_LEN * 0.35,
    deckTop + fairingH / 2,
    0,
  );
  fairing.castShadow = true;
  group.add(fairing);

  // Rounded front face (cabless aero nose)
  const frontBulge = new THREE.Mesh(
    new THREE.SphereGeometry(W * 0.48, 24, 18, 0, Math.PI, 0, Math.PI),
    fairingMat,
  );
  frontBulge.scale.set(0.55, fairingH / (W * 0.96), 0.95);
  frontBulge.rotation.y = Math.PI / 2;
  frontBulge.position.set(L / 2 + NOSE_LEN * 0.72, deckTop + fairingH / 2, 0);
  frontBulge.castShadow = true;
  group.add(frontBulge);

  // Amber marker lights along top of fairing
  const amber = mat(0xe8a838, {
    roughness: 0.35,
    metalness: 0.2,
    emissive: 0xe8a838,
    emissiveIntensity: 0.55,
  });
  for (const z of [-0.7, -0.25, 0.25, 0.7]) {
    box(0.08, 0.06, 0.1, amber, L / 2 + NOSE_LEN * 0.55, deckTop + fairingH - 0.08, z, group);
  }

  // Front LED bars (low)
  const led = mat(COLORS.led, {
    roughness: 0.2,
    metalness: 0.1,
    emissive: 0xffffff,
    emissiveIntensity: 0.9,
  });
  box(0.05, 0.08, 0.55, led, L / 2 + NOSE_LEN - 0.08, y0 + 0.08, 0.55, group);
  box(0.05, 0.08, 0.55, led, L / 2 + NOSE_LEN - 0.08, y0 + 0.08, -0.55, group);
  // Vertical marker strips on fairing
  box(0.04, 0.55, 0.05, led, L / 2 + NOSE_LEN * 0.85, y0 + 0.35, W * 0.38, group);
  box(0.04, 0.55, 0.05, led, L / 2 + NOSE_LEN * 0.85, y0 + 0.35, -W * 0.38, group);

  // Sensor pods on fairing shoulders
  addSensorPod(group, L / 2 + NOSE_LEN * 0.55, deckTop + fairingH * 0.35, W * 0.48);
  addSensorPod(group, L / 2 + NOSE_LEN * 0.55, deckTop + fairingH * 0.35, -W * 0.48);

  // Central camera cluster
  const lens = mat(COLORS.sensorLens, { metalness: 0.6, roughness: 0.2 });
  const cam = new THREE.Mesh(new THREE.SphereGeometry(0.09, 16, 12), lens);
  cam.position.set(L / 2 + NOSE_LEN * 0.95, y0 + 0.15, 0);
  group.add(cam);
  const camHi = new THREE.Mesh(new THREE.SphereGeometry(0.07, 14, 12), lens);
  camHi.position.set(L / 2 + NOSE_LEN * 0.7, deckTop + fairingH * 0.55, 0);
  group.add(camHi);
}

function addWheelPair(parent, x) {
  const radius = 0.48;
  const width = 0.28;
  const tireMat = mat(COLORS.tire, { roughness: 0.9, metalness: 0.05 });
  const rimMat = mat(COLORS.rim, { roughness: 0.35, metalness: 0.55 });
  const zOff = W * 0.42;

  for (const side of [-1, 1]) {
    const z = side * zOff;
    // Tire
    const tire = new THREE.Mesh(
      new THREE.CylinderGeometry(radius, radius, width, 28),
      tireMat,
    );
    tire.rotation.z = Math.PI / 2;
    tire.position.set(x, radius, z);
    tire.castShadow = true;
    parent.add(tire);

    // Rim disc
    const rim = new THREE.Mesh(
      new THREE.CylinderGeometry(radius * 0.62, radius * 0.62, width * 0.35, 24),
      rimMat,
    );
    rim.rotation.z = Math.PI / 2;
    rim.position.set(x, radius, z + side * width * 0.15);
    parent.add(rim);

    // Tri-spoke
    for (let i = 0; i < 3; i++) {
      const spoke = new THREE.Mesh(
        new THREE.BoxGeometry(0.08, radius * 0.95, 0.06),
        rimMat,
      );
      spoke.position.set(x, radius, z + side * width * 0.22);
      spoke.rotation.x = (i * Math.PI * 2) / 3;
      parent.add(spoke);
    }
  }

  // Axle
  box(
    0.08,
    0.08,
    zOff * 2,
    mat(COLORS.darkMetal, { metalness: 0.6 }),
    x,
    radius,
    0,
    parent,
    { cast: false },
  );
}

function addSensorPod(parent, x, y, z) {
  const body = mat(COLORS.darkMetal, { metalness: 0.4, roughness: 0.35 });
  const accent = mat(COLORS.sensor, {
    roughness: 0.4,
    metalness: 0.2,
    emissive: COLORS.sensor,
    emissiveIntensity: 0.25,
  });
  const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.05, 0.12, 10), body);
  stem.position.set(x, y + 0.06, z);
  parent.add(stem);
  const head = new THREE.Mesh(new THREE.SphereGeometry(0.09, 14, 12), accent);
  head.position.set(x, y + 0.16, z);
  head.castShadow = true;
  parent.add(head);
  const lens = new THREE.Mesh(
    new THREE.SphereGeometry(0.04, 10, 8),
    mat(COLORS.sensorLens, { metalness: 0.7, roughness: 0.15 }),
  );
  lens.position.set(x + Math.sign(x || 1) * 0.05, y + 0.16, z);
  parent.add(lens);
}

function buildBody(group) {
  const deckTop = GROUND_CLEAR + CHASSIS_H + 0.06;
  const bodyY = deckTop + H / 2;

  const shellMat = mat(COLORS.body, { roughness: 0.5, metalness: 0.12 });
  const lightMat = mat(COLORS.bodyLight, { roughness: 0.55, metalness: 0.08 });
  const corrMat = mat(COLORS.corrugation, { roughness: 0.6, metalness: 0.1 });

  // Main shell — solid box for exterior; hidden in cutaway so interior reads
  const shell = box(L, H, W, shellMat, 0, bodyY, 0, group);
  shell.userData.cutawayHide = true;

  // Open shell faces shown only in cutaway (no near +z wall)
  const openFloor = box(
    L * 0.98,
    0.06,
    W * 0.96,
    shellMat,
    0,
    deckTop + 0.03,
    0,
    group,
  );
  openFloor.userData.cutawayOnly = true;
  openFloor.visible = false;
  const openRoof = box(
    L * 0.98,
    0.06,
    W * 0.96,
    lightMat,
    0,
    deckTop + H - 0.03,
    0,
    group,
  );
  openRoof.userData.cutawayOnly = true;
  openRoof.visible = false;
  const openFar = box(
    L * 0.98,
    H * 0.96,
    0.06,
    shellMat,
    0,
    bodyY,
    -W / 2 + 0.03,
    group,
  );
  openFar.userData.cutawayOnly = true;
  openFar.visible = false;
  const openFront = box(
    0.06,
    H * 0.96,
    W * 0.96,
    shellMat,
    L / 2 - 0.03,
    bodyY,
    0,
    group,
  );
  openFront.userData.cutawayOnly = true;
  openFront.visible = false;
  const openRear = box(
    0.06,
    H * 0.96,
    W * 0.96,
    shellMat,
    -L / 2 + 0.03,
    bodyY,
    0,
    group,
  );
  openRear.userData.cutawayOnly = true;
  openRear.visible = false;

  // Corrugation ribs on long sides
  const ribCount = 28;
  const ribPitch = L / ribCount;
  for (let i = 0; i < ribCount; i++) {
    const x = -L / 2 + ribPitch * (i + 0.5);
    for (const z of [W / 2 + 0.01, -W / 2 - 0.01]) {
      const rib = box(
        ribPitch * 0.55,
        H * 0.96,
        0.035,
        corrMat,
        x,
        bodyY,
        z,
        group,
        { cast: false },
      );
      // Hide near-side (+z, toward default camera) in cutaway
      if (z > 0) rib.userData.cutawayHide = true;
    }
  }

  // Corner castings (container ISO corners)
  const cornerMat = mat(COLORS.metal, { metalness: 0.75, roughness: 0.35 });
  for (const x of [-L / 2 + 0.1, L / 2 - 0.1]) {
    for (const z of [-W / 2 + 0.1, W / 2 - 0.1]) {
      for (const y of [deckTop + 0.1, deckTop + H - 0.1]) {
        box(0.18, 0.18, 0.18, cornerMat, x, y, z, group);
      }
    }
  }

  // Vertical corner posts
  for (const x of [-L / 2 + 0.06, L / 2 - 0.06]) {
    for (const z of [-W / 2 + 0.06, W / 2 - 0.06]) {
      const post = box(0.1, H, 0.1, cornerMat, x, bodyY, z, group);
      if (z > 0) post.userData.cutawayHide = true;
    }
  }

  // Roof rails + solar array
  box(L * 0.98, 0.06, W * 0.98, lightMat, 0, deckTop + H + 0.02, 0, group);
  const solarMat = mat(COLORS.solar, { roughness: 0.25, metalness: 0.45 });
  for (let i = 0; i < 5; i++) {
    const x = -L / 2 + 1.4 + i * 2.35;
    box(2.1, 0.04, W * 0.72, solarMat, x, deckTop + H + 0.06, 0, group);
  }

  // Windows (RV living adaptations of the box)
  const glassMat = mat(COLORS.glass, {
    roughness: 0.08,
    metalness: 0.05,
    transparent: true,
    opacity: 0.72,
    emissive: COLORS.glassTint,
    emissiveIntensity: 0.28,
  });
  const frameMat = mat(COLORS.darkMetal, { roughness: 0.45, metalness: 0.35 });

  // Side windows (far side always; near side cutaway-hide)
  const windowSpecs = [
    { x: 3.5, w: 2.4, h: 1.2 }, // lounge
    { x: 0.8, w: 1.8, h: 1.0 }, // kitchen
    { x: -1.8, w: 1.1, h: 0.65 }, // wet cell high
    { x: -4.0, w: 2.0, h: 1.0 }, // bedroom
  ];
  for (const win of windowSpecs) {
    for (const side of [-1, 1]) {
      const z = side * (W / 2 + 0.025);
      const frame = box(
        win.w + 0.1,
        win.h + 0.1,
        0.05,
        frameMat,
        win.x,
        deckTop + 1.5,
        z,
        group,
        { cast: false },
      );
      const glass = box(
        win.w,
        win.h,
        0.06,
        glassMat,
        win.x,
        deckTop + 1.5,
        z + side * 0.02,
        group,
        { cast: false },
      );
      if (side > 0) {
        frame.userData.cutawayHide = true;
        glass.userData.cutawayHide = true;
      }
    }
  }

  // Entry door (near side, mid-front)
  const doorMat = mat(COLORS.bodyLight, { roughness: 0.5, metalness: 0.1 });
  const door = box(1.0, 2.1, 0.08, doorMat, 2.2, deckTop + 1.1, W / 2 + 0.03, group);
  door.userData.cutawayHide = true;
  // Door glass
  const doorGlass = box(
    0.55,
    0.9,
    0.04,
    glassMat,
    2.2,
    deckTop + 1.55,
    W / 2 + 0.08,
    group,
    { cast: false },
  );
  doorGlass.userData.cutawayHide = true;

  // Rear doors (container-style double doors adapted for RV)
  const rearMat = mat(COLORS.body, { roughness: 0.5, metalness: 0.15 });
  box(0.08, H * 0.92, W * 0.45, rearMat, -L / 2 - 0.02, bodyY, W * 0.24, group);
  box(0.08, H * 0.92, W * 0.45, rearMat, -L / 2 - 0.02, bodyY, -W * 0.24, group);
  // Lock rods
  const rodMat = mat(COLORS.metal, { metalness: 0.7, roughness: 0.3 });
  for (const z of [0.35, -0.35]) {
    box(0.04, H * 0.85, 0.04, rodMat, -L / 2 - 0.06, bodyY, z, group);
  }

  // Fossall wordmark plate
  const plate = box(
    1.6,
    0.35,
    0.03,
    mat(COLORS.chassisAccent, {
      roughness: 0.4,
      metalness: 0.2,
      emissive: COLORS.chassisAccent,
      emissiveIntensity: 0.15,
    }),
    0,
    deckTop + H - 0.4,
    W / 2 + 0.03,
    group,
  );
  plate.userData.cutawayHide = true;

}

function buildInterior(group) {
  const deckTop = GROUND_CLEAR + CHASSIS_H + 0.06;
  const wall = 0.08;
  const innerW = W - wall * 2;
  const innerH = H - 0.2;

  // Floor
  box(
    L - 0.2,
    0.05,
    innerW,
    mat(COLORS.floor, { roughness: 0.75, metalness: 0.05 }),
    0,
    deckTop + 0.03,
    0,
    group,
    { cast: false },
  );

  // Ceiling
  box(
    L - 0.2,
    0.04,
    innerW,
    mat(COLORS.interior, { roughness: 0.8 }),
    0,
    deckTop + innerH,
    0,
    group,
    { cast: false },
  );

  // Far wall (keeps enclosure in cutaway)
  box(
    L - 0.15,
    innerH - 0.1,
    0.05,
    mat(COLORS.interior, { roughness: 0.85 }),
    0,
    deckTop + innerH / 2,
    -innerW / 2 + 0.02,
    group,
    { cast: false },
  );

  // ── Zones along length: lounge (+x) · galley · wet · sleep (-x) ──

  // Lounge bench
  const furn = mat(COLORS.furniture, { roughness: 0.7, metalness: 0.05 });
  box(2.0, 0.45, 0.55, furn, 4.2, deckTop + 0.28, -0.4, group);
  box(0.55, 0.45, 1.6, furn, 5.0, deckTop + 0.28, 0.15, group);
  // Table
  box(
    0.7,
    0.05,
    0.7,
    mat(COLORS.floor, { roughness: 0.5 }),
    4.0,
    deckTop + 0.72,
    0.2,
    group,
  );
  box(0.08, 0.65, 0.08, furn, 4.0, deckTop + 0.38, 0.2, group);

  // Galley counter
  const wetMat = mat(COLORS.wet, { roughness: 0.4, metalness: 0.15 });
  box(2.2, 0.9, 0.55, wetMat, 1.2, deckTop + 0.5, -innerW / 2 + 0.35, group);
  // Sink dimple
  box(0.4, 0.08, 0.35, mat(COLORS.metal, { metalness: 0.8 }), 1.5, deckTop + 0.96, -innerW / 2 + 0.35, group);
  // Upper cabinets
  box(2.2, 0.45, 0.35, furn, 1.2, deckTop + 2.2, -innerW / 2 + 0.25, group);

  // Wet cell enclosure
  const wetWall = mat(0xb8c4c2, { roughness: 0.6 });
  box(1.4, 2.0, 0.06, wetWall, -1.5, deckTop + 1.05, 0.2, group);
  box(0.06, 2.0, 1.4, wetWall, -0.85, deckTop + 1.05, -0.3, group);
  // Shower base
  box(1.1, 0.1, 1.1, wetMat, -1.5, deckTop + 0.1, -0.35, group);

  // Bed platform (rear)
  box(2.4, 0.4, innerW * 0.9, furn, -4.3, deckTop + 0.28, 0, group);
  // Mattress
  box(
    2.2,
    0.18,
    innerW * 0.82,
    mat(0xf2ebe0, { roughness: 0.9 }),
    -4.3,
    deckTop + 0.55,
    0,
    group,
  );
  // Headboard
  box(0.08, 0.7, innerW * 0.85, furn, -5.4, deckTop + 0.85, 0, group);

  // Storage under bed lip
  box(2.3, 0.15, innerW * 0.88, mat(COLORS.bodyLight), -4.3, deckTop + 0.12, 0, group);

  // Partition bulkhead lounge/galley
  box(0.05, 1.6, innerW * 0.5, furn, 2.8, deckTop + 0.9, -0.3, group);

  // Soft cabin light strips
  const cabinLed = mat(0xfff8e8, {
    roughness: 0.4,
    metalness: 0,
    emissive: 0xfff0d0,
    emissiveIntensity: 0.6,
  });
  box(L * 0.85, 0.03, 0.04, cabinLed, 0, deckTop + innerH - 0.08, -0.3, group);
  box(L * 0.85, 0.03, 0.04, cabinLed, 0, deckTop + innerH - 0.08, 0.3, group);
}

function buildDimensionLabels(group) {
  const deckTop = GROUND_CLEAR + CHASSIS_H + 0.06;
  const y = 0.05;

  // Length bar along +z side
  addDimBar(group, L, 0, y, W / 2 + 1.1, 0, `40′ · ${L.toFixed(2)} m`);
  // Width bar at front
  addDimBar(group, W, L / 2 + NOSE_LEN + 0.6, y, 0, Math.PI / 2, `${W.toFixed(2)} m wide`);
  // Height bar
  addDimBar(
    group,
    H + CHASSIS_H + GROUND_CLEAR,
    -L / 2 - 0.9,
    (H + CHASSIS_H + GROUND_CLEAR) / 2,
    W / 2 + 0.8,
    Math.PI / 2,
    `~${(H + CHASSIS_H + GROUND_CLEAR).toFixed(1)} m high`,
    true,
  );

  // Zone labels on ground
  const zones = [
    { x: 4.0, text: "Lounge" },
    { x: 1.2, text: "Galley" },
    { x: -1.5, text: "Wet" },
    { x: -4.3, text: "Sleep" },
  ];
  for (const z of zones) {
    const sprite = makeTextSprite(z.text, {
      fontSize: 48,
      color: "#0b5f4a",
      bg: "rgba(247,244,239,0.85)",
    });
    sprite.position.set(z.x, 0.02, W / 2 + 1.8);
    sprite.scale.set(1.4, 0.4, 1);
    group.add(sprite);
  }

  const title = makeTextSprite("Fossall 40′ EV-RV sketch", {
    fontSize: 56,
    color: "#1a1a1a",
    bg: "rgba(247,244,239,0.9)",
  });
  title.position.set(0, deckTop + H + 1.2, 0);
  title.scale.set(4.5, 0.7, 1);
  group.add(title);

  const sub = makeTextSprite("cabless chassis · container envelope · living module", {
    fontSize: 40,
    color: "#5c5c5c",
    bg: "rgba(247,244,239,0.85)",
  });
  sub.position.set(0, deckTop + H + 0.7, 0);
  sub.scale.set(5.2, 0.45, 1);
  group.add(sub);
}

function addDimBar(parent, length, x, y, z, rotY, label, vertical = false) {
  const lineMat = new THREE.MeshBasicMaterial({
    color: COLORS.chassisAccent,
    transparent: true,
    opacity: 0.75,
  });
  const thick = 0.025;
  let bar;
  if (vertical) {
    bar = new THREE.Mesh(new THREE.BoxGeometry(thick, length, thick), lineMat);
    bar.position.set(x, y, z);
  } else {
    bar = new THREE.Mesh(new THREE.BoxGeometry(length, thick, thick), lineMat);
    bar.position.set(x, y, z);
    bar.rotation.y = rotY;
  }
  parent.add(bar);

  // End caps
  for (const t of [-0.5, 0.5]) {
    const cap = new THREE.Mesh(
      new THREE.BoxGeometry(thick * 1.2, thick * 4, thick * 1.2),
      lineMat,
    );
    if (vertical) {
      cap.position.set(x, y + t * length, z);
    } else {
      const dx = Math.cos(rotY) * t * length;
      const dz = -Math.sin(rotY) * t * length;
      cap.position.set(x + dx, y, z + dz);
    }
    parent.add(cap);
  }

  const sprite = makeTextSprite(label, {
    fontSize: 42,
    color: "#0b5f4a",
    bg: "rgba(238,246,243,0.92)",
  });
  if (vertical) {
    sprite.position.set(x - 0.4, y, z);
    sprite.scale.set(2.2, 0.4, 1);
  } else {
    sprite.position.set(x, y + 0.25, z + (rotY === 0 ? 0.15 : 0));
    sprite.scale.set(2.8, 0.4, 1);
  }
  parent.add(sprite);
}

function makeTextSprite(text, opts = {}) {
  const fontSize = opts.fontSize || 48;
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  const pad = 24;
  ctx.font = `600 ${fontSize}px system-ui, sans-serif`;
  const metrics = ctx.measureText(text);
  canvas.width = Math.ceil(metrics.width + pad * 2);
  canvas.height = Math.ceil(fontSize * 1.5 + pad);
  ctx.font = `600 ${fontSize}px system-ui, sans-serif`;
  ctx.fillStyle = opts.bg || "rgba(255,255,255,0.9)";
  roundRect(ctx, 0, 0, canvas.width, canvas.height, 16);
  ctx.fill();
  ctx.fillStyle = opts.color || "#111";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(text, canvas.width / 2, canvas.height / 2);

  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  const mat = new THREE.SpriteMaterial({
    map: tex,
    transparent: true,
    depthTest: true,
  });
  return new THREE.Sprite(mat);
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

// Optional auto-mount when loaded as a classic module script.
// Preferred path is the essay page bootstrap calling mountRvModel().
function boot() {
  const host = document.getElementById("rv-model");
  if (!host || host.dataset.mounted === "1") return;
  host.dataset.mounted = "1";
  mountRvModel(host);
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
  document.body?.addEventListener("htmx:afterSettle", boot);
}
