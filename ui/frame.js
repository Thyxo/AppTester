// Mockup-ramme omkring en app. Layoutet beregnes i Rust (appwin/device.rs);
// her tegnes kun enheden, skærmens statuslinje m.m. og værktøjslinjen.

const { invoke } = window.__TAURI__.core;
const appWindow = window.__TAURI__.window.getCurrentWindow();
const label = new URLSearchParams(location.search).get("label");

const $ = (id) => document.getElementById(id);

const ICONS = {
  reload: '<path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/>',
  "hard-reload": '<path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/>',
  "clear-data": '<path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>',
  devtools: '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>',
  rotate: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
  "open-in-browser": '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  minimize: '<path d="M5 12h14"/>',
  close: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
};
const svg = (name) => `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;

/** Sideknapper i portræt-koordinater (pt): kant, position langs kanten, længde. */
const BUTTONS = {
  "iphone-15-pro": [["left", 118, 30], ["left", 176, 58], ["left", 246, 58], ["right", 200, 96]],
  "iphone-15-pro-max": [["left", 126, 32], ["left", 188, 62], ["left", 262, 62], ["right", 214, 102]],
  "iphone-se": [["left", 100, 28], ["left", 160, 56], ["left", 230, 56], ["right", 160, 56]],
  "pixel-8": [["right", 190, 62], ["right", 280, 112]],
  "ipad-air": [["top", 640, 64], ["right", 90, 48], ["right", 146, 48]],
};

let state = null;

// ---------- Hjælpere ----------

function place(el, x, y, w, h) {
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
  el.style.width = `${w}px`;
  el.style.height = `${h}px`;
}

/** Lys eller mørk tekst, der kan læses på `hex`. */
function contrastOn(hex) {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})/i.exec(hex || "");
  if (!m) return "#000";
  let h = m[1];
  if (h.length === 3) h = [...h].map((c) => c + c).join("");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#000" : "#fff";
}

function statusIcons(size) {
  // Signal, wifi og batteri – tegnet efter iOS.
  const h = size;
  return `
    <svg viewBox="0 0 18 12" style="height:${h * 0.72}px;margin-right:${h * 0.3}px">
      <rect x="0" y="8" width="3" height="4" rx="1"/><rect x="5" y="5.5" width="3" height="6.5" rx="1"/>
      <rect x="10" y="3" width="3" height="9" rx="1"/><rect x="15" y="0" width="3" height="12" rx="1"/>
    </svg>
    <svg viewBox="0 0 16 12" style="height:${h * 0.72}px;margin-right:${h * 0.3}px">
      <path d="M8 2.3c2.4 0 4.6.9 6.2 2.5l1.2-1.2C13.5 1.7 10.9.6 8 .6S2.5 1.7.6 3.6l1.2 1.2C3.4 3.2 5.6 2.3 8 2.3zm0 3.4c1.5 0 2.8.6 3.8 1.5L13 6c-1.3-1.3-3.1-2-5-2s-3.7.7-5 2l1.2 1.2c1-.9 2.3-1.5 3.8-1.5zm0 3.4c.6 0 1.1.2 1.5.6L8 11.4 6.5 9.7c.4-.4.9-.6 1.5-.6z"/>
    </svg>
    <svg viewBox="0 0 27 13" style="height:${h * 0.78}px">
      <rect x=".5" y=".5" width="23" height="12" rx="3.5" fill="none" stroke="currentColor" opacity=".4"/>
      <rect x="2" y="2" width="20" height="9" rx="2"/>
      <path d="M25 4.5v4c.8-.3 1.3-1.1 1.3-2s-.5-1.7-1.3-2z" opacity=".4"/>
    </svg>`;
}

function updateClock() {
  const now = new Date();
  const sep = /^(da|nb|fi)/i.test(navigator.language || "") ? "." : ":";
  $("clock").textContent = `${now.getHours()}${sep}${String(now.getMinutes()).padStart(2, "0")}`;
}

// ---------- Kabinet ----------

function renderShell(l) {
  const shell = $("shell");
  const s = l.scale;
  shell.hidden = false;
  shell.className = `drag ${l.kind} ${l.notch}`;

  // Kabinettet tegnes i portræt; i landskab drejes det 90° mod uret.
  const W = l.landscape ? l.phone.h : l.phone.w;
  const H = l.landscape ? l.phone.w : l.phone.h;
  const cx = l.phone.x + l.phone.w / 2;
  const cy = l.phone.y + l.phone.h / 2;
  place(shell, cx - W / 2, cy - H / 2, W, H);
  shell.style.transform = l.landscape ? "rotate(-90deg)" : "";
  shell.style.borderRadius = `${l.radius}px`;

  // Skærmens placering i portræt-koordinater.
  const sx = l.landscape ? l.screen.y - l.phone.y : l.screen.x - l.phone.x;
  const sy = l.landscape ? l.screen.x - l.phone.x : l.screen.y - l.phone.y;
  const sw = l.landscape ? l.screen.h : l.screen.w;
  const sh = l.landscape ? l.screen.w : l.screen.h;

  const rim = Math.max(1, l.rim);
  const bezel = $("bezel");
  place(bezel, rim, rim, W - 2 * rim, H - 2 * rim);
  bezel.style.borderRadius = `${Math.max(0, l.radius - rim)}px`;

  const glass = $("glass");
  place(glass, sx, sy, sw, sh);
  glass.style.borderRadius = `${l.screenRadius}px`;

  // Desktop: vinduets titellinje over skærmen.
  if (l.kind === "desktop") {
    place($("titlebar"), 0, 0, W, sy);
    $("titlebar").style.borderRadius = `${l.radius}px ${l.radius}px 0 0`;
    $("titlebar-name").textContent = state?.name ?? "";
  }

  // iPhone SE: højttaler og kamera over skærmen, hjemknap under.
  const topBand = sy;
  const bottomBand = H - sy - sh;
  if (l.notch === "home-button") {
    place($("speaker"), W / 2 - 25 * s, topBand / 2 - 3 * s, 50 * s, 6 * s);
    place($("camera"), W / 2 - 45 * s, topBand / 2 - 5 * s, 10 * s, 10 * s);
    const hb = 58 * s;
    place($("homebutton"), W / 2 - hb / 2, sy + sh + bottomBand / 2 - hb / 2, hb, hb);
  } else if (l.notch === "camera") {
    place($("camera"), W / 2 - 4.5 * s, topBand / 2 - 4.5 * s, 9 * s, 9 * s);
  }

  // Sideknapper stikker lidt ud af kabinettet.
  const t = Math.max(2, 3.5 * s);
  const buttons = $("buttons");
  buttons.replaceChildren(
    ...(BUTTONS[l.device] ?? []).map(([edge, at, len]) => {
      const b = document.createElement("i");
      if (edge === "left") place(b, -t + 0.5, at * s, t, len * s);
      else if (edge === "right") place(b, W - 0.5, at * s, t, len * s);
      else place(b, at * s, -t + 0.5, len * s, t);
      return b;
    })
  );
}

// ---------- Skærm-overlag (statuslinje, hjem-indikator, Dynamic Island) ----------

function renderOverlay(l) {
  const overlay = $("overlay");
  overlay.hidden = false;
  const s = l.scale;
  const c = state?.colors ?? { statusBar: "#ffffff", background: "#ffffff" };
  const i = l.insets;
  const scr = l.screen;

  // Statuslinje i den øverste afstand.
  const bar = $("statusbar");
  const iphone = l.notch === "island";
  bar.style.display = i.top > 0 ? "flex" : "none";
  if (i.top > 0) {
    place(bar, scr.x, scr.y, scr.w, i.top);
    bar.style.background = c.statusBar;
    bar.style.color = contrastOn(c.statusBar);
    bar.style.borderRadius = `${l.screenRadius}px ${l.screenRadius}px 0 0`;
    const fs = (iphone ? 16 : 12.5) * s;
    bar.style.fontSize = `${fs}px`;
    // iPhone: tid og ikoner centreret i "ørerne" på hver side af øen.
    const ear = iphone ? scr.w * 0.13 : 16 * s;
    bar.style.padding = `${iphone ? i.top * 0.12 : 0}px ${ear}px 0`;
    bar.classList.toggle("centered", l.notch === "home-button");
    $("sb-icons").innerHTML = statusIcons(fs);
    $("sb-icons").style.display = l.notch === "home-button" ? "none" : "flex";
  }

  // Siderne i landskab (fx iPhone: plads til øen).
  const left = $("fill-left");
  const right = $("fill-right");
  const r = i.top > 0 ? 0 : l.screenRadius;
  for (const [el, w, x] of [[left, i.left, scr.x], [right, i.right, scr.x + scr.w - i.right]]) {
    el.style.display = w > 0 ? "block" : "none";
    if (w > 0) {
      place(el, x, scr.y + i.top, w, scr.h - i.top - i.bottom);
      el.style.background = c.background;
    }
  }
  left.style.borderRadius = `${r}px 0 0 0`;
  right.style.borderRadius = `0 ${r}px 0 0`;

  // Hjem-indikator i den nederste afstand.
  const home = $("homebar");
  home.style.display = i.bottom > 0 ? "grid" : "none";
  if (i.bottom > 0) {
    place(home, scr.x, scr.y + scr.h - i.bottom, scr.w, i.bottom);
    home.style.background = c.background;
    home.style.borderRadius = `0 0 ${l.screenRadius}px ${l.screenRadius}px`;
    const pill = home.firstElementChild;
    const android = l.notch === "punch";
    pill.style.width = `${(android ? 110 : l.kind === "tablet" ? 300 : 134) * s}px`;
    pill.style.height = `${(android ? 4 : 5) * s}px`;
    pill.style.background = contrastOn(c.background);
    pill.style.opacity = ".85";
  }

  // Hjem-indikator oven på appen (Windows: appens webview har et hul her).
  const ind = $("indicator");
  ind.style.display = l.indicator ? "block" : "none";
  if (l.indicator) {
    place(ind, l.indicator.x, l.indicator.y, l.indicator.w, l.indicator.h);
    ind.style.background = contrastOn(c.background);
  }

  // Dynamic Island / hulkamera.
  const island = $("island");
  island.className = "";
  island.style.display = "none";
  if (l.island) {
    // I fuld skærm har appen et hul her, så øen ses oven på appen.
    island.style.display = "block";
    place(island, l.island.x, l.island.y, l.island.w, l.island.h);
  } else if (l.notch === "punch") {
    island.style.display = "block";
    island.className = "punch";
    const d = 13 * s;
    if (l.landscape) place(island, scr.x + (i.left - d) / 2, scr.y + scr.h / 2 - d / 2, d, d);
    else place(island, scr.x + scr.w / 2 - d / 2, scr.y + (i.top - d) / 2, d, d);
  }
}

function renderLayout(l) {
  renderShell(l);
  renderOverlay(l);
}

// ---------- Status ----------

function renderStatus(st) {
  const dep = $("dep");
  const preview = st.deployment === "preview";
  dep.textContent = preview ? "PREVIEW" : "PROD";
  dep.className = "chip " + (preview ? "preview" : "prod");
  dep.disabled = !state?.hasPreview;
  dep.title = state?.hasPreview
    ? "Skift mellem production og preview"
    : "Tilføj en preview-adresse for at kunne skifte";

  const sw = $("sw");
  sw.textContent = { ok: "SW ✓", registered: "SW registreret", none: "Ingen SW" }[st.sw] ?? "";
  sw.className = "chip" + (st.sw === "ok" ? " ok" : st.sw === "registered" ? " warn" : "");
  sw.title = {
    ok: "Siden styres af en service worker – kan køre offline",
    registered: "Service worker registreret, men styrer ikke siden endnu",
    none: "Ingen service worker",
  }[st.sw] ?? "";

  const net = $("net");
  net.textContent = { online: "Online", "offline-cache": "Offline · cache", offline: "Offline" }[st.net] ?? "";
  net.className = "chip" + (st.net === "offline-cache" ? " warn" : st.net === "offline" ? " bad" : "");

  if (state) state.status = st;
}

// Rust kalder denne via eval.
window.__frame = (kind, data) => {
  if (kind === "layout") {
    if (state) state.layout = data;
    renderLayout(data);
    syncDeviceControls();
  } else if (kind === "status") {
    renderStatus(data);
  }
};

function syncDeviceControls() {
  if (!state) return;
  $("device").value = state.layout.device;
  const dev = state.devices.find((d) => d.id === state.layout.device);
  $("rotate").disabled = !dev?.canRotate;
  const [vw, vh] = state.layout.viewport;
  $("device").title = `Enhed – appen ser ${Math.round(vw)}×${Math.round(vh)} px`;
}

// ---------- Handlinger ----------

async function act(action) {
  try {
    await invoke("frame_action", { label, action });
  } catch (err) {
    console.error(err);
  }
}

for (const btn of document.querySelectorAll("[data-action]")) {
  btn.innerHTML = svg(btn.dataset.action);
  btn.addEventListener("click", () => act(btn.dataset.action));
}
for (const btn of document.querySelectorAll("[data-win]")) {
  btn.innerHTML = svg(btn.dataset.win);
  btn.addEventListener("click", () =>
    btn.dataset.win === "close" ? appWindow.close() : appWindow.minimize()
  );
}

$("dep").addEventListener("click", () => {
  const next = state?.status?.deployment === "preview" ? "production" : "preview";
  act(`deploy:${next}`);
});
$("device").addEventListener("change", (e) => act(`device:${e.target.value}`));

// Træk vinduet i værktøjslinjen eller kabinettet.
for (const el of document.querySelectorAll(".drag")) {
  el.addEventListener("mousedown", (e) => {
    if (e.button !== 0 || e.target.closest("button, select")) return;
    appWindow.startDragging();
  });
}
$("grip").addEventListener("mousedown", (e) => {
  if (e.button === 0) appWindow.startResizeDragging("SouthEast");
});

// Temaskift (fra launcheren) ændrer kabinettets metal og værktøjslinjen.
window.addEventListener("themechange", () => state && renderLayout(state.layout));

// ---------- Start ----------

async function init(attempt = 0) {
  try {
    state = await invoke("frame_init", { label });
  } catch (err) {
    // Vinduet kan være ved at blive sat op – prøv igen et øjeblik.
    if (attempt < 20) return setTimeout(() => init(attempt + 1), 100);
    $("name").textContent = String(err);
    return;
  }
  $("name").textContent = state.name;
  document.title = state.name;
  $("device").replaceChildren(...state.devices.map((d) => new Option(d.name, d.id)));
  renderLayout(state.layout);
  syncDeviceControls();
  renderStatus(state.status);
}

updateClock();
setInterval(updateClock, 15000);
init();
