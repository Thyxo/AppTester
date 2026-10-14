const { invoke } = window.__TAURI__.core;

const $ = (id) => document.getElementById(id);
const els = {
  search: $("search"),
  sort: $("sort"),
  refresh: $("refresh"),
  error: $("error"),
  offline: $("offline"),
  favSection: $("fav-section"),
  favGrid: $("fav-grid"),
  grid: $("grid"),
  allTitle: $("all-title"),
  empty: $("empty"),
  nomatch: $("nomatch"),
};

let apps = [];
/** Fejl fra seneste ikon-hentning pr. app-id. */
const metaErrors = new Map();

// ---------- Hjælpere ----------

function showError(message) {
  els.error.textContent = message;
  els.error.hidden = !message;
}

const prefs = {
  get(key, fallback) {
    try { return localStorage.getItem(key) ?? fallback; } catch { return fallback; }
  },
  set(key, value) {
    try { localStorage.setItem(key, value); } catch { /* ikke vigtigt */ }
  },
};

// Stabil farve pr. app, når der ikke er et ikon.
function hue(id) {
  let h = 2166136261;
  for (const c of id) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return ((h >>> 0) % 12) * 30;
}

// ---------- Visning ----------

function tile(app) {
  const li = document.createElement("li");
  li.className = "cell";

  const btn = document.createElement("button");
  btn.className = "tile";
  btn.dataset.id = app.id;
  const err = metaErrors.get(app.id);
  btn.title = err && !app.meta ? `${app.url}\n${err}` : app.url;
  if (app.meta?.themeColor) btn.style.setProperty("--tile-accent", app.meta.themeColor);

  let icon;
  if (app.meta?.icon) {
    icon = document.createElement("img");
    icon.src = app.meta.icon;
    icon.alt = "";
  } else {
    icon = document.createElement("span");
    icon.classList.add("letter");
    icon.style.background = `hsl(${hue(app.id)} 65% 45%)`;
    icon.textContent = app.name.trim().charAt(0).toUpperCase();
  }
  icon.classList.add("icon");

  const name = document.createElement("span");
  name.className = "name";
  name.textContent = app.name;

  const host = document.createElement("span");
  host.className = "host";
  host.textContent = new URL(app.url).host;
  if (err && !app.meta) {
    host.textContent = "⚠ " + err;
    host.classList.add("warn");
  }

  btn.append(icon, name, host);
  btn.addEventListener("click", () => openApp(app, btn));

  const star = document.createElement("button");
  star.className = "star" + (app.favorite ? " on" : "");
  star.textContent = app.favorite ? "★" : "☆";
  star.title = app.favorite ? "Fjern fra favoritter" : "Tilføj til favoritter";
  star.setAttribute("aria-pressed", String(app.favorite));
  star.addEventListener("click", () => toggleFavorite(app));

  li.append(btn, star);
  return li;
}

function visibleApps() {
  const q = els.search.value.trim().toLowerCase();
  const list = apps.filter((a) =>
    !q ||
    a.name.toLowerCase().includes(q) ||
    a.id.includes(q) ||
    (a.meta?.name ?? "").toLowerCase().includes(q) ||
    a.url.toLowerCase().includes(q)
  );
  const byName = (a, b) => a.name.localeCompare(b.name, "da");
  if (els.sort.value === "recent") {
    list.sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0) || byName(a, b));
  } else {
    list.sort(byName);
  }
  return list;
}

function render() {
  const list = visibleApps();
  const favs = list.filter((a) => a.favorite);
  const rest = list.filter((a) => !a.favorite);

  els.favGrid.replaceChildren(...favs.map(tile));
  els.grid.replaceChildren(...rest.map(tile));
  els.favSection.hidden = favs.length === 0;
  els.allTitle.textContent = favs.length ? "Øvrige" : "Apps";
  els.allTitle.parentElement.hidden = rest.length === 0;
  els.empty.hidden = apps.length > 0;
  els.nomatch.hidden = apps.length === 0 || list.length > 0;
}

// ---------- Handlinger ----------

async function load() {
  try {
    apps = await invoke("list_apps");
    showError("");
  } catch (err) {
    showError(String(err));
    apps = [];
  }
  render();
}

async function openApp(app, btn) {
  showError("");
  btn.disabled = true;
  try {
    await invoke("open_app", { id: app.id });
    app.lastOpened = Date.now();
    if (els.sort.value === "recent") render();
  } catch (err) {
    showError(String(err));
  } finally {
    btn.disabled = false;
  }
}

async function toggleFavorite(app) {
  try {
    app.favorite = await invoke("toggle_favorite", { id: app.id });
    render();
  } catch (err) {
    showError(String(err));
  }
}

/** Henter manifest/ikon for alle apps, højst 4 ad gangen. */
async function refreshAll() {
  els.refresh.disabled = true;
  let ok = 0;
  let offline = 0;
  const queue = [...apps];
  async function worker() {
    for (let app = queue.shift(); app; app = queue.shift()) {
      try {
        app.meta = await invoke("refresh_meta", { id: app.id });
        metaErrors.delete(app.id);
        ok++;
      } catch (err) {
        const msg = String(err);
        metaErrors.set(app.id, msg);
        if (msg.includes("offline") || msg.includes("forbindelse")) offline++;
      }
      render();
    }
  }
  await Promise.all(Array.from({ length: 4 }, worker));
  els.offline.hidden = !(ok === 0 && offline > 0);
  els.refresh.disabled = false;
}

// ---------- Opsætning ----------

els.sort.value = prefs.get("sort", "name");
els.sort.addEventListener("change", () => {
  prefs.set("sort", els.sort.value);
  render();
});
els.search.addEventListener("input", render);
els.search.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    document.querySelector(".tile")?.click();
  } else if (e.key === "Escape") {
    els.search.value = "";
    render();
  }
});
els.refresh.addEventListener("click", refreshAll);

document.addEventListener("keydown", (e) => {
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
  if ((e.key === "/" && !typing) || (e.key === "f" && (e.ctrlKey || e.metaKey))) {
    e.preventDefault();
    els.search.focus();
    els.search.select();
  }
});

load().then(refreshAll);
