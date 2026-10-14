const { invoke } = window.__TAURI__.core;

const grid = document.getElementById("grid");
const errorBox = document.getElementById("error");
const empty = document.getElementById("empty");

function showError(message) {
  errorBox.textContent = message;
  errorBox.hidden = false;
}

// Stabil farve pr. app, indtil rigtige ikoner kommer fra manifestet.
function hue(id) {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
}

function tile(app) {
  const li = document.createElement("li");
  const btn = document.createElement("button");
  btn.className = "tile";
  btn.title = app.url;

  const icon = document.createElement("span");
  icon.className = "icon";
  icon.style.background = `hsl(${hue(app.id)} 65% 45%)`;
  icon.textContent = app.name.trim().charAt(0).toUpperCase();

  const name = document.createElement("span");
  name.className = "name";
  name.textContent = app.name;

  const host = document.createElement("span");
  host.className = "host";
  host.textContent = new URL(app.url).host;

  btn.append(icon, name, host);
  btn.addEventListener("click", async () => {
    errorBox.hidden = true;
    btn.disabled = true;
    try {
      await invoke("open_app", { id: app.id });
    } catch (err) {
      showError(String(err));
    } finally {
      btn.disabled = false;
    }
  });

  li.append(btn);
  return li;
}

async function load() {
  try {
    const apps = await invoke("list_apps");
    apps.sort((a, b) => a.name.localeCompare(b.name, "da"));
    grid.replaceChildren(...apps.map(tile));
    empty.hidden = apps.length > 0;
  } catch (err) {
    showError(String(err));
  }
}

load();
