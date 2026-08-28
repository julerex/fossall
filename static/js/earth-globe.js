/**
 * Deep-time Earth globe — reconstructed continents on a Three.js sphere.
 * Drag to orbit · scroll to zoom · slider sets age (Ma ago).
 */

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

const OCEAN = "#0b3a4a";
const LAND = "#c4b896";
const LAND_EDGE = "#8a7a5c";
const FOSSIL = 0xf0c14a;
const MAX_MA = 1800;
const TEX_W = 1024;
const TEX_H = 512;

/**
 * @param {HTMLElement} host
 */
export function mountEarthGlobe(host) {
  const canvas = host.querySelector("canvas");
  const status = host.querySelector("[data-earth-status]");
  const slider = host.querySelector("#earth-ma");
  const readout = host.querySelector("[data-earth-readout]");
  const search = host.querySelector("#earth-taxon");
  const results = host.querySelector("[data-earth-results]");
  if (!canvas) return;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x07090c);

  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: true,
    alpha: false,
    powerPreference: "high-performance",
  });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 40);
  camera.position.set(0.4, 0.8, 3.4);

  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = true;
  controls.dampingFactor = 0.06;
  controls.minDistance = 1.7;
  controls.maxDistance = 8;
  controls.enablePan = false;
  controls.autoRotate = true;
  controls.autoRotateSpeed = 0.35;

  const texCanvas = document.createElement("canvas");
  texCanvas.width = TEX_W;
  texCanvas.height = TEX_H;
  const texCtx = texCanvas.getContext("2d");
  fillOcean(texCtx);
  const texture = new THREE.CanvasTexture(texCanvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.anisotropy = 4;

  const globe = new THREE.Mesh(
    new THREE.SphereGeometry(1, 64, 48),
    new THREE.MeshPhongMaterial({
      map: texture,
      specular: new THREE.Color(0x223344),
      shininess: 12,
    }),
  );
  scene.add(globe);

  const atmos = new THREE.Mesh(
    new THREE.SphereGeometry(1.035, 32, 24),
    new THREE.MeshBasicMaterial({
      color: 0x6ec8ff,
      transparent: true,
      opacity: 0.12,
      side: THREE.BackSide,
    }),
  );
  scene.add(atmos);

  scene.add(new THREE.AmbientLight(0xffffff, 0.55));
  const sun = new THREE.DirectionalLight(0xfff4e0, 1.15);
  sun.position.set(4, 1.4, 2.2);
  scene.add(sun);

  const fossilGeom = new THREE.BufferGeometry();
  fossilGeom.setAttribute("position", new THREE.BufferAttribute(new Float32Array(0), 3));
  const fossils = new THREE.Points(
    fossilGeom,
    new THREE.PointsMaterial({
      color: FOSSIL,
      size: 0.018,
      sizeAttenuation: true,
    }),
  );
  scene.add(fossils);

  /** @type {{units: {name:string, rank:string, start_ma:number, end_ma:number, color_hex?:string}[]}} */
  let timescale = { units: [] };
  /** @type {number|null} */
  let selectedTaxon = null;
  let selectedName = "";
  let loadGen = 0;

  function setStatus(msg) {
    if (status) status.textContent = msg;
  }

  function resize() {
    const w = host.clientWidth || 640;
    const h = host.clientHeight || 420;
    renderer.setSize(w, h, false);
    camera.aspect = w / Math.max(h, 1);
    camera.updateProjectionMatrix();
  }
  resize();
  window.addEventListener("resize", resize);

  function render() {
    controls.update();
    renderer.render(scene, camera);
    requestAnimationFrame(render);
  }
  requestAnimationFrame(render);

  function periodName(ma) {
    const periods = timescale.units.filter((u) => u.rank === "period");
    const hit = periods.find((u) => u.start_ma >= ma && u.end_ma <= ma);
    if (hit) return hit.name;
    const any = timescale.units.find((u) => u.start_ma >= ma && u.end_ma <= ma);
    return hit ? hit.name : any ? any.name : "";
  }

  function updateReadout(ma, intervalName) {
    if (!readout) return;
    const name = intervalName || periodName(ma);
    if (ma <= 0.05) {
      readout.textContent = name ? `0 Ma · ${name} (present)` : "0 Ma · present";
    } else {
      readout.textContent = name ? `${formatMa(ma)} Ma · ${name}` : `${formatMa(ma)} Ma ago`;
    }
  }

  async function loadTimescale() {
    try {
      const res = await fetch("/api/earth/timescale");
      if (!res.ok) throw new Error(await errorMessage(res));
      timescale = await res.json();
    } catch (err) {
      setStatus(String(err.message || err));
    }
  }

  async function loadContinents(ma) {
    const gen = ++loadGen;
    setStatus("Loading reconstruction…");
    try {
      const res = await fetch(`/api/earth/continents?ma=${encodeURIComponent(ma)}`);
      if (!res.ok) throw new Error(await errorMessage(res));
      const data = await res.json();
      if (gen !== loadGen) return;
      paintGeojson(texCtx, data);
      texture.needsUpdate = true;
      const intervalName = data.interval && data.interval.name;
      updateReadout(data.time_ma ?? ma, intervalName);
      const n = (data.features && data.features.length) || 0;
      setStatus(
        n
          ? `${n} land polygons at ${formatMa(data.time_ma)} Ma`
          : "No reconstruction for this age yet. Seed with make seed-earth.",
      );
      await loadOccurrences(data.time_ma ?? ma);
    } catch (err) {
      if (gen !== loadGen) return;
      setStatus(String(err.message || err));
    }
  }

  async function loadOccurrences(ma) {
    const params = new URLSearchParams({ ma: String(ma), limit: "1500" });
    if (selectedTaxon) params.set("taxon_id", String(selectedTaxon));
    try {
      const res = await fetch(`/api/earth/occurrences?${params}`);
      if (!res.ok) {
        setFossilPoints([]);
        return;
      }
      const data = await res.json();
      setFossilPoints(data.occurrences || []);
      if (selectedName && data.count != null) {
        setStatus(`${selectedName}: ${data.count} occurrences at ${formatMa(ma)} Ma`);
      }
    } catch {
      setFossilPoints([]);
    }
  }

  function setFossilPoints(occs) {
    const pts = [];
    for (const o of occs) {
      if (typeof o.paleolat !== "number" || typeof o.paleolng !== "number") continue;
      const v = latLngToVec(o.paleolat, o.paleolng, 1.01);
      pts.push(v.x, v.y, v.z);
    }
    fossilGeom.setAttribute("position", new THREE.BufferAttribute(new Float32Array(pts), 3));
    fossilGeom.computeBoundingSphere();
  }

  let searchTimer = 0;
  if (search) {
    search.addEventListener("input", () => {
      clearTimeout(searchTimer);
      const q = search.value.trim();
      if (q.length < 2) {
        selectedTaxon = null;
        selectedName = "";
        if (results) {
          results.hidden = true;
          results.innerHTML = "";
        }
        return;
      }
      searchTimer = window.setTimeout(() => lookupTaxa(q), 200);
    });
  }

  async function lookupTaxa(q) {
    try {
      const res = await fetch(`/api/earth/taxa?q=${encodeURIComponent(q)}`);
      if (!res.ok) return;
      const data = await res.json();
      renderResults(data.taxa || []);
    } catch {
      /* ignore */
    }
  }

  function renderResults(taxa) {
    if (!results) return;
    results.innerHTML = "";
    if (!taxa.length) {
      results.hidden = true;
      return;
    }
    for (const t of taxa) {
      const li = document.createElement("li");
      const btn = document.createElement("button");
      btn.type = "button";
      const rank = t.rank ? ` (${t.rank})` : "";
      btn.textContent = `${t.scientific_name}${rank}`;
      btn.addEventListener("click", () => {
        selectedTaxon = t.id;
        selectedName = t.scientific_name;
        if (search) search.value = t.scientific_name;
        results.hidden = true;
        let ma = Number(slider && slider.value) || 0;
        const fad = Number(t.first_app_ma);
        const lad = Number(t.last_app_ma);
        if (Number.isFinite(fad) && Number.isFinite(lad) && (ma > fad || ma < lad)) {
          ma = Math.round((fad + lad) / 2);
          if (slider) {
            slider.value = String(ma);
            slider.setAttribute("aria-valuenow", String(ma));
          }
          loadContinents(ma);
          return;
        }
        loadOccurrences(ma);
      });
      li.appendChild(btn);
      results.appendChild(li);
    }
    results.hidden = false;
  }

  if (slider) {
    slider.addEventListener("input", () => {
      const ma = Number(slider.value) || 0;
      slider.setAttribute("aria-valuenow", String(ma));
      updateReadout(ma);
    });
    slider.addEventListener("change", () => {
      loadContinents(Number(slider.value) || 0);
    });
  }

  loadTimescale().then(() => loadContinents(0));
}

function formatMa(ma) {
  if (ma < 0.05) return "0";
  if (ma >= 10) return String(Math.round(ma));
  if (ma >= 1) return ma.toFixed(1);
  return ma.toFixed(2);
}

async function errorMessage(res) {
  try {
    const body = await res.json();
    if (body && body.error) return body.error;
  } catch {
    /* ignore */
  }
  return `HTTP ${res.status}`;
}

function fillOcean(ctx) {
  ctx.fillStyle = OCEAN;
  ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
}

function paintGeojson(ctx, fc) {
  fillOcean(ctx);
  const features = (fc && fc.features) || [];
  ctx.lineWidth = 1;
  ctx.strokeStyle = LAND_EDGE;
  ctx.fillStyle = LAND;
  for (const f of features) {
    const g = f.geometry;
    if (!g) continue;
    if (g.type === "Polygon") drawPolygon(ctx, g.coordinates);
    else if (g.type === "MultiPolygon") {
      for (const poly of g.coordinates) drawPolygon(ctx, poly);
    } else if (g.type === "LineString") drawLine(ctx, g.coordinates);
  }
}

function drawPolygon(ctx, rings) {
  if (!rings || !rings.length) return;
  ctx.beginPath();
  pathRing(ctx, rings[0]);
  for (let i = 1; i < rings.length; i += 1) pathRing(ctx, rings[i]);
  ctx.fill("evenodd");
  ctx.stroke();
}

function drawLine(ctx, pts) {
  if (!pts || pts.length < 2) return;
  ctx.beginPath();
  pathRing(ctx, pts);
  ctx.stroke();
}

function pathRing(ctx, ring) {
  const w = ctx.canvas.width;
  const h = ctx.canvas.height;
  let started = false;
  let prevX = null;
  for (const pt of ring) {
    const lon = pt[0];
    const lat = pt[1];
    if (typeof lon !== "number" || typeof lat !== "number") continue;
    const x = ((lon + 180) / 360) * w;
    const y = ((90 - lat) / 180) * h;
    if (!started) {
      ctx.moveTo(x, y);
      started = true;
    } else if (prevX != null && Math.abs(x - prevX) > w * 0.5) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
    prevX = x;
  }
}

function latLngToVec(lat, lng, r) {
  const phi = (90 - lat) * (Math.PI / 180);
  const theta = (lng + 180) * (Math.PI / 180);
  return new THREE.Vector3(
    -r * Math.sin(phi) * Math.cos(theta),
    r * Math.cos(phi),
    r * Math.sin(phi) * Math.sin(theta),
  );
}
