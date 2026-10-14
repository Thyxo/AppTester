// Mockup-ramme omkring en app. Layoutet beregnes i Rust (appwin/device.rs);
// her tegnes kun telefonen og værktøjslinjen efter det.

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

let state = null;

// ---------- Tegning ----------

function place(el, x, y, w, h) {
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
  el.style.width = `${w}px`;
  el.style.height = `${h}px`;
}

function renderLayout(l) {
  const phone = $("phone");
  const s = l.scale;
  phone.hidden = false;
  phone.classList.toggle("desktop", l.device === "desktop");
  place(phone, l.phone.x, l.phone.y, l.phone.w, l.phone.h);
  phone.style.borderRadius = `${l.radius}px`;

  // Alt herunder er relativt til telefonen.
  const sx = l.screen.x - l.phone.x;
  const sy = l.screen.y - l.phone.y;
  const screen = $("screen");
  place(screen, sx, sy, l.screen.w, l.screen.h);
  screen.style.borderRadius = l.device === "desktop" ? `${l.radius}px` : `${Math.max(2, 6 * s)}px`;

  // "Top"-kanten er venstre side i landskab.
  const topBand = l.landscape
    ? { x: 0, y: 0, w: sx, h: l.phone.h }
    : { x: 0, y: 0, w: l.phone.w, h: sy };
  const bottomBand = l.landscape
    ? { x: sx + l.screen.w, y: 0, w: l.phone.w - sx - l.screen.w, h: l.phone.h }
    : { x: 0, y: sy + l.screen.h, w: l.phone.w, h: l.phone.h - sy - l.screen.h };
  const center = (band, w, h) =>
    l.landscape
      ? [band.x + (band.w - h) / 2, band.y + (band.h - w) / 2, h, w]
      : [band.x + (band.w - w) / 2, band.y + (band.h - h) / 2, w, h];

  const notch = $("notch");
  const home = $("home");
  notch.className = "";
  home.className = "";
  switch (l.notch) {
    case "island":
      notch.className = "island";
      place(notch, ...center(topBand, 118 * s, 32 * s));
      home.className = "indicator";
      place(home, ...center(bottomBand, 130 * s, 5 * s));
      break;
    case "home-button":
      notch.className = "speaker";
      place(notch, ...center(topBand, 54 * s, 6 * s));
      home.className = "button";
      place(home, ...center(bottomBand, 56 * s, 56 * s));
      break;
    case "punch":
      notch.className = "punch";
      place(notch, ...center(topBand, 14 * s, 14 * s));
      home.className = "indicator";
      place(home, ...center(bottomBand, 110 * s, 4 * s));
      break;
    case "camera":
      notch.className = "camera";
      place(notch, ...center(topBand, 9 * s, 9 * s));
      break;
  }

  // Sideknapper (lydstyrke + tænd/sluk) på telefoner.
  const phoneLike = l.notch === "island" || l.notch === "punch" || l.notch === "home-button";
  const btns = [$("btn-a"), $("btn-b"), $("btn-c")];
  btns.forEach((b) => b.classList.toggle("on", phoneLike));
  if (phoneLike) {
    const t = 3; // knappens tykkelse
    const spec = [
      { side: "start", at: 150, len: 56 },
      { side: "start", at: 220, len: 56 },
      { side: "end", at: 180, len: 90 },
    ];
    spec.forEach((b, i) => {
      const at = b.at * s;
      const len = b.len * s;
      if (l.landscape) {
        // Telefonen drejet mod uret: venstre side → bund, højre side → top.
        const y = b.side === "start" ? l.phone.h : -t;
        place(btns[i], at, y, len, t);
      } else {
        const x = b.side === "start" ? -t : l.phone.w;
        place(btns[i], x, at, t, len);
      }
    });
  }
}

function renderStatus(st) {
  const dep = $("dep");
  dep.textContent = st.deployment === "preview" ? "PREVIEW" : "PROD";
  dep.className = "chip" + (st.deployment === "preview" ? " preview" : "");
  dep.disabled = !state?.hasPreview;
  dep.title = state?.hasPreview
    ? "Skift mellem production og preview"
    : 'Tilføj en "preview"-adresse for at kunne skifte';

  const sw = $("sw");
  const swText = { ok: "SW ✓", registered: "SW registreret", none: "Ingen SW" }[st.sw] ?? "";
  sw.textContent = swText;
  sw.className = "chip" + (st.sw === "ok" ? " ok" : st.sw === "registered" ? " warn" : "");
  sw.title = {
    ok: "Siden styres af en service worker – kan køre offline",
    registered: "Service worker registreret, men styrer ikke siden endnu",
    none: "Ingen service worker",
  }[st.sw] ?? "";

  const net = $("net");
  net.textContent = { online: "Online", "offline-cache": "Offline · cache", offline: "Offline" }[st.net] ?? "";
  net.className = "chip" + (st.net === "offline-cache" ? " warn" : st.net === "offline" ? " bad" : "");

  state && (state.status = st);
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
  const select = $("device");
  select.value = state.layout.device;
  const dev = state.devices.find((d) => d.id === state.layout.device);
  $("rotate").disabled = !dev?.canRotate;
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

// Træk vinduet i værktøjslinjen eller telefonens ramme.
for (const el of document.querySelectorAll(".drag")) {
  el.addEventListener("mousedown", (e) => {
    if (e.button !== 0 || e.target.closest("button, select")) return;
    appWindow.startDragging();
  });
}
$("grip").addEventListener("mousedown", (e) => {
  if (e.button === 0) appWindow.startResizeDragging("SouthEast");
});

// ---------- Start ----------

(async () => {
  try {
    state = await invoke("frame_init", { label });
  } catch (err) {
    $("name").textContent = String(err);
    return;
  }
  $("name").textContent = state.name;
  document.title = state.name;
  $("device").replaceChildren(
    ...state.devices.map((d) => new Option(d.name, d.id))
  );
  renderLayout(state.layout);
  syncDeviceControls();
  renderStatus(state.status);
})();
