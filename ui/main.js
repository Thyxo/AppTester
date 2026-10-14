const { invoke } = window.__TAURI__.core;

const $ = (id) => document.getElementById(id);
const els = {
  greeting: $("greeting"),
  themeBtn: $("theme-btn"),
  refresh: $("refresh"),
  openSettings: $("open-settings"),
  search: $("search"),
  stats: document.querySelectorAll(".stat[data-filter]"),
  countAll: $("count-all"),
  countFav: $("count-fav"),
  countRecent: $("count-recent"),
  error: $("error"),
  offline: $("offline"),
  grid: $("grid"),
  empty: $("empty"),
  nomatch: $("nomatch"),
  addApp: $("add-app"),
  addDialog: $("add-dialog"),
  addForm: $("add-form"),
  addUrl: $("add-url"),
  addName: $("add-name"),
  addPreview: $("add-preview"),
  addError: $("add-error"),
  addSubmit: $("add-submit"),
  settings: $("settings"),
  themeList: $("theme-list"),
  settingsList: $("settings-list"),
  settingsEmpty: $("settings-empty"),
  editApps: $("edit-apps"),
};

const ICONS = {
  search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
  refresh: '<path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/>',
  settings: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
  palette: '<circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/><circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.93 0 1.65-.75 1.65-1.69 0-.44-.18-.84-.44-1.13-.29-.29-.44-.65-.44-1.13a1.64 1.64 0 0 1 1.67-1.67h2c3.05 0 5.55-2.5 5.55-5.55C21.97 6.01 17.46 2 12 2z"/>',
  arrow: '<path d="M7 17 17 7"/><path d="M7 7h10v10"/>',
};
const svg = (name) => `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;

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

let filter = prefs.get("filter", "all");

/** Kortfarve (1–4): skiftevis efter appens plads i den samlede liste (stabil ved filtrering). */
let colorOf = new Map();
function assignColors() {
  const ids = apps.map((a) => a.id).sort();
  colorOf = new Map(ids.map((id, i) => [id, (i % 4) + 1]));
}
const colorIndex = (id) => colorOf.get(id) ?? 1;

function relativeTime(ms) {
  if (!ms) return "Aldrig åbnet";
  const min = Math.round((Date.now() - ms) / 60000);
  if (min < 1) return "Åbnet lige nu";
  if (min < 60) return `Åbnet for ${min} min. siden`;
  const hours = Math.round(min / 60);
  if (hours < 24) return `Åbnet for ${hours} t. siden`;
  const days = Math.round(hours / 24);
  if (days === 1) return "Åbnet i går";
  if (days < 30) return `Åbnet for ${days} dage siden`;
  return `Åbnet ${new Date(ms).toLocaleDateString("da-DK")}`;
}

function greeting() {
  const h = new Date().getHours();
  if (h < 5) return "God nat";
  if (h < 10) return "God morgen";
  if (h < 12) return "God formiddag";
  if (h < 18) return "God eftermiddag";
  return "God aften";
}

// ---------- Visning ----------

function card(app) {
  const li = document.createElement("li");
  li.className = `card c${colorIndex(app.id)}`;
  li.tabIndex = 0;
  li.setAttribute("role", "button");
  li.setAttribute("aria-label", `Åbn ${app.name}`);
  li.title = app.url;

  const head = document.createElement("div");
  head.className = "card-head";

  const icon = document.createElement("span");
  icon.className = "card-icon";
  if (app.meta?.icon) {
    const img = document.createElement("img");
    img.src = app.meta.icon;
    img.alt = "";
    icon.append(img);
  } else {
    const letter = document.createElement("span");
    letter.className = "letter";
    letter.textContent = app.name.trim().charAt(0).toUpperCase();
    icon.append(letter);
  }

  const text = document.createElement("span");
  text.className = "card-text";
  const name = document.createElement("span");
  name.className = "card-name";
  name.textContent = app.name;
  if (app.preview) {
    const badge = document.createElement("span");
    badge.className = "card-badge";
    badge.textContent = "preview";
    badge.title = "Skift i app-vinduets værktøjslinje (PROD/PREVIEW)";
    name.append(badge);
  }
  const host = document.createElement("span");
  host.className = "card-host";
  host.textContent = new URL(app.url).host;
  text.append(name, host);
  head.append(icon, text);

  const foot = document.createElement("div");
  foot.className = "card-foot";
  const meta = document.createElement("span");
  meta.className = "card-meta";
  const err = metaErrors.get(app.id);
  if (err && !app.meta) {
    meta.textContent = "⚠ " + err;
    meta.classList.add("warn");
  } else {
    meta.textContent = relativeTime(app.lastOpened);
  }
  const go = document.createElement("button");
  go.className = "card-go";
  go.type = "button";
  go.tabIndex = -1;
  go.setAttribute("aria-hidden", "true");
  go.innerHTML = svg("arrow");
  foot.append(meta, go);

  const star = document.createElement("button");
  star.type = "button";
  star.className = "card-star" + (app.favorite ? " on" : "");
  star.textContent = app.favorite ? "★" : "☆";
  star.title = app.favorite ? "Fjern fra favoritter" : "Tilføj til favoritter";
  star.setAttribute("aria-pressed", String(app.favorite));
  star.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleFavorite(app);
  });

  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "card-remove";
  remove.textContent = "✕";
  remove.title = `Fjern ${app.name} fra listen`;
  remove.addEventListener("click", (e) => {
    e.stopPropagation();
    removeApp(app);
  });

  li.append(head, foot, star, remove);
  li.addEventListener("click", () => openApp(app, li));
  li.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openApp(app, li);
    }
  });
  return li;
}

function visibleApps() {
  const q = els.search.value.trim().toLowerCase();
  const byName = (a, b) => a.name.localeCompare(b.name, "da");
  let list = apps.filter((a) =>
    !q ||
    a.name.toLowerCase().includes(q) ||
    a.id.includes(q) ||
    (a.meta?.name ?? "").toLowerCase().includes(q) ||
    a.url.toLowerCase().includes(q)
  );
  if (filter === "fav") {
    list = list.filter((a) => a.favorite).sort(byName);
  } else if (filter === "recent") {
    list = list.filter((a) => a.lastOpened).sort((a, b) => b.lastOpened - a.lastOpened);
  } else {
    list.sort((a, b) => Number(b.favorite) - Number(a.favorite) || byName(a, b));
  }
  return list;
}

function render() {
  els.countAll.textContent = apps.length;
  els.countFav.textContent = apps.filter((a) => a.favorite).length;
  els.countRecent.textContent = apps.filter((a) => a.lastOpened).length;
  for (const s of els.stats) {
    const active = s.dataset.filter === filter;
    s.classList.toggle("active", active);
    s.setAttribute("aria-pressed", String(active));
  }

  const list = visibleApps();
  els.grid.replaceChildren(...list.map(card));
  els.empty.hidden = apps.length > 0 || !els.error.hidden;
  els.nomatch.hidden = apps.length === 0 || list.length > 0;
  els.nomatch.textContent = els.search.value.trim()
    ? "Ingen apps matcher søgningen."
    : filter === "fav"
      ? "Ingen favoritter endnu – klik ☆ på et kort."
      : "Du har ikke åbnet nogen apps endnu.";
}

// ---------- Handlinger ----------

async function load() {
  try {
    apps = await invoke("list_apps");
    assignColors();
    showError("");
  } catch (err) {
    showError(String(err));
    apps = [];
  }
  render();
}

async function openApp(app, el) {
  if (el.classList.contains("opening")) return;
  showError("");
  el.classList.add("opening");
  try {
    await invoke("open_app", { id: app.id });
    app.lastOpened = Date.now();
    render();
  } catch (err) {
    showError(String(err));
  } finally {
    el.classList.remove("opening");
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

/** Henter manifest/ikon for apps (standard: alle), højst 4 ad gangen. */
async function refreshAll(list = apps) {
  if (list.length === 0) return;
  els.refresh.disabled = true;
  let ok = 0;
  let offline = 0;
  const queue = [...list];
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
  // Kun en fuld opdatering, hvor intet lykkedes, betyder sandsynligvis offline.
  if (list === apps) els.offline.hidden = !(ok === 0 && offline > 0);
  els.refresh.disabled = false;
}

// ---------- Tilføj / fjern ----------

function openAddDialog() {
  els.addForm.reset();
  els.addError.hidden = true;
  els.addSubmit.disabled = false;
  els.addSubmit.textContent = "Tilføj";
  els.addDialog.showModal();
  els.addUrl.focus();
}

async function submitAdd(e) {
  e.preventDefault();
  els.addError.hidden = true;
  els.addSubmit.disabled = true;
  els.addSubmit.textContent = "Henter app…";
  try {
    const id = await invoke("add_app", {
      url: els.addUrl.value,
      name: els.addName.value.trim() || null,
      preview: els.addPreview.value.trim() || null,
    });
    els.addDialog.close();
    if (filter !== "all") setFilter("all");
    await load();
    const added = apps.find((a) => a.id === id);
    if (added && !added.meta) refreshAll([added]);
  } catch (err) {
    els.addError.textContent = String(err);
    els.addError.hidden = false;
    els.addSubmit.disabled = false;
    els.addSubmit.textContent = "Tilføj";
  }
}

async function removeApp(app) {
  if (!confirm(`Fjern ${app.name} fra listen?\n\nAppens gemte data bevares, hvis du tilføjer den igen.`)) return;
  try {
    await invoke("remove_app", { id: app.id });
    await load();
  } catch (err) {
    showError(String(err));
  }
}

async function editApps() {
  try {
    await invoke("edit_apps_file");
  } catch (err) {
    showError(String(err));
  }
}

/** Genindlæser listen (fx efter redigering af apps.json) og henter ikoner til nye apps. */
async function reload() {
  const known = new Set(apps.map((a) => a.id));
  await load();
  refreshAll(apps.filter((a) => !known.has(a.id) && !a.meta));
}

function setFilter(next) {
  filter = next;
  prefs.set("filter", next);
  render();
}

// ---------- Indstillinger ----------

function renderThemes() {
  const current = window.getTheme();
  els.themeList.replaceChildren(
    ...window.PWA_THEMES.map((t) => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "theme";
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(t.id === current));
      const sw = document.createElement("span");
      sw.className = "swatch";
      for (const c of t.swatch) {
        const i = document.createElement("i");
        i.style.background = c;
        sw.append(i);
      }
      const label = document.createElement("span");
      label.textContent = t.name;
      b.append(sw, label);
      b.addEventListener("click", () => {
        window.setTheme(t.id);
        renderThemes();
      });
      return b;
    })
  );
}

function settingsRow(app) {
  const li = document.createElement("li");

  const title = document.createElement("strong");
  title.textContent = app.name;
  const url = document.createElement("span");
  url.className = "url";
  url.textContent = app.preview;

  const row = document.createElement("div");
  row.className = "row";
  const input = document.createElement("input");
  input.type = "password";
  input.autocomplete = "off";
  input.placeholder = app.hasBypass ? "•••••••• (gemt)" : "Bypass-token";
  const save = document.createElement("button");
  save.type = "button";
  save.className = "primary";
  save.textContent = "Gem";
  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "ghost";
  remove.textContent = "Fjern";
  remove.disabled = !app.hasBypass;
  row.append(input, save, remove);

  const status = document.createElement("span");
  status.setAttribute("role", "status");

  async function store(token) {
    status.className = "";
    status.textContent = "";
    try {
      app.hasBypass = await invoke("set_bypass_token", { id: app.id, token });
      input.value = "";
      input.placeholder = app.hasBypass ? "•••••••• (gemt)" : "Bypass-token";
      remove.disabled = !app.hasBypass;
      status.className = "status-ok";
      status.textContent = app.hasBypass ? "✓ Gemt i keychain" : "✓ Fjernet";
    } catch (err) {
      status.className = "status-err";
      status.textContent = String(err);
    }
  }
  save.addEventListener("click", () => {
    if (input.value.trim()) store(input.value);
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && input.value.trim()) store(input.value);
  });
  remove.addEventListener("click", () => store(""));

  li.append(title, url, row, status);
  return li;
}

function openSettings() {
  renderThemes();
  const withPreview = apps.filter((a) => a.preview);
  withPreview.sort((a, b) => a.name.localeCompare(b.name, "da"));
  els.settingsList.replaceChildren(...withPreview.map(settingsRow));
  els.settingsEmpty.hidden = withPreview.length > 0;
  els.settings.showModal();
}

function cycleTheme() {
  const list = window.PWA_THEMES;
  const i = list.findIndex((t) => t.id === window.getTheme());
  const next = list[(i + 1) % list.length];
  window.setTheme(next.id);
  els.themeBtn.title = `Tema: ${next.name} (klik for at skifte)`;
}

// ---------- Opsætning ----------

els.greeting.textContent = greeting();
els.themeBtn.innerHTML = svg("palette");
els.refresh.innerHTML = svg("refresh");
els.openSettings.innerHTML = svg("settings");
document.querySelector(".search-icon").innerHTML = svg("search");

for (const s of els.stats) s.addEventListener("click", () => setFilter(s.dataset.filter));
els.search.addEventListener("input", render);
els.search.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    document.querySelector(".card")?.click();
  } else if (e.key === "Escape") {
    els.search.value = "";
    render();
  }
});
els.themeBtn.addEventListener("click", cycleTheme);
els.refresh.addEventListener("click", () => refreshAll());
els.openSettings.addEventListener("click", openSettings);
els.editApps.addEventListener("click", editApps);
els.addApp.addEventListener("click", openAddDialog);
els.addForm.addEventListener("submit", submitAdd);
for (const b of document.querySelectorAll("[data-close]")) {
  b.addEventListener("click", () => b.closest("dialog").close());
}
window.addEventListener("focus", reload);

document.addEventListener("keydown", (e) => {
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement;
  if ((e.key === "/" && !typing) || (e.key === "f" && (e.ctrlKey || e.metaKey))) {
    e.preventDefault();
    els.search.focus();
    els.search.select();
  }
});

// Opdater "Åbnet for … siden" af og til.
setInterval(render, 60000);

load().then(() => refreshAll());
