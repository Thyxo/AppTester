# PWA Launcher

Native desktop-app (Rust + Tauri 2) til at åbne og teste mine webapps/PWAs som
"rigtige" apps – ét vindue pr. app, egen vedvarende storage, testværktøjer.

WebView2 på Windows, WebKit på macOS/Linux. Ingen Electron.

## Status

| Trin | Indhold | Status |
|---|---|---|
| a | App-vindue med egen data-mappe, eksterne links i browser | ✅ |
| b | App-liste i `apps.json` + launcher-vindue | ✅ |
| c | Manifest-ikoner/farver, søg, sortering, favoritter, husk vinduesstørrelse | ✅ |
| d | Testmenu: DevTools, viewports, hard reload, ryd data, preview, SW-/offline-status | ✅ |

## Kom i gang

**Krav**
- Rust (stable) – <https://rustup.rs>
- Windows: WebView2 (følger med Windows 10/11) og "Desktop development with C++" fra Visual Studio Build Tools
- macOS: Xcode Command Line Tools
- Linux: `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev`
- Tauri CLI: `cargo install tauri-cli --version "^2" --locked`

**Kør / byg**

```sh
cargo tauri dev      # kør i udviklingstilstand
cargo tauri build    # installer i src-tauri/target/release/bundle/
```

Uden Tauri CLI: `cd src-tauri && cargo run`.

Hvert push bygger desuden på Windows, macOS og Linux i GitHub Actions
(`.github/workflows/build.yml`). Windows-jobbet uploader `.msi` + `.exe` som artefakt.

## Tilføj en app

Klik **+ Tilføj app**, indsæt linket (fx `monizzz.vercel.app`) og tryk **Tilføj**.
Navn og ikon hentes automatisk fra appens manifest. Valgfrit: eget navn og en
preview-adresse. Fjern en app med **✕** på flisen.

Listen gemmes i `apps.json` i config-mappen (se *Hvor ligger data?*). Den kan også
redigeres manuelt under **⚙ → Rediger apps.json manuelt** – også flere lister
efter hinanden (`[...] [...]`) accepteres og ryddes op ved næste ændring.

## Launcheren

- **Ikoner og farver** hentes fra appens web manifest og caches, så griddet ser
  ens ud offline. **↻** henter igen.
- **Søg**: `/` eller `Ctrl+F`, `Enter` åbner første app, `Esc` rydder.
- **Sortering**: navn eller senest åbnet. **☆** gør en app til favorit.

## App-vinduet: enheds-mockup

Hver app åbner i en telefonramme (iPhone 15 som standard) uden Windows-ramme:

- **Forholdet er låst.** Vinduet kan ikke blive større end enhedens 100 %-størrelse.
  Gør du det mindre (grebet nederst til højre), skaleres hele telefonen ned – appen
  ser stadig præcis enhedens viewport (fx 393×852), bare zoomet.
- **Flyt** vinduet ved at trække i værktøjslinjen eller telefonens ramme.
- Vinduet husker størrelse, placering og valgt enhed pr. app.

**Værktøjslinjen**

| Knap | Hvad det gør |
|---|---|
| PROD / PREVIEW | Skift mellem production og preview (hvis appen har en preview-adresse) |
| SW ✓ · Online | Status: service worker styrer siden / netværk (offline – kører fra cache) |
| ⟳ | Genindlæs |
| ⚡ | Hard reload: afregistrerer service workers og tømmer Cache Storage |
| 🗑 | Ryd alle data for appen (med bekræftelse) |
| `<>` | DevTools for appen |
| Enhed | iPhone 15 (393×852) · iPhone SE (375×667) · Pixel 8 (412×915) · iPad (820×1180) · Desktop (1280×800) |
| ⟲ | Vend (portræt/landskab) |
| ↗ | Åbn i standardbrowser |

User-agent og touch ændres ikke – kun viewport-størrelsen.

På **Linux** kan Tauri ikke placere to webviews frit i samme vindue; dér åbnes appen
i et almindeligt vindue med enhedens viewport, uden ramme og værktøjslinje.

### Beskyttede previews

Har projektet Vercel Deployment Protection, så lav et token under
**Vercel → projekt → Settings → Deployment Protection → Protection Bypass for Automation**,
og indsæt det i launcheren under **⚙**. Tokenet gemmes i OS'ets keychain
(Windows Credential Manager / macOS Keychain / Secret Service), aldrig i en fil.

## Arkitektur

```
Launcher-vindue (ui/, vanilla JS)
        │ invoke()
Rust-kerne (src-tauri/src)
  apps · manifest · store · appwin
        │ WebviewWindowBuilder
App-vinduer (ét pr. app, gennemsigtigt, uden systemramme)
  frame-webview (lokal: telefon + værktøjslinje)
  content-webview (appen, egen data-mappe, ingen IPC) lagt over skærmen
```

| Valg | Løsning |
|---|---|
| Storage pr. app | `data_directory` (Windows/Linux), `data_store_identifier` (macOS 14+) |
| Eksterne links | `on_navigation` / `on_new_window` → standardbrowser |
| Redirects | Indtil første side er indlæst, følges redirects (fx til eget domæne eller Vercel-login) |
| Mockup | To webviews i ét vindue (Tauri `unstable`); layout beregnes i `appwin/device.rs`, appen zoomes til enhedens viewport |
| Rettigheder | Kun launcher-vinduet må kalde Rust-kommandoerne; webapps i app-vinduer har ingen IPC-adgang |

## Filstruktur

```
AppTester/
├─ README.md
├─ apps.json                     standard-app-liste (kopieres ud første gang)
├─ .github/workflows/build.yml   CI: Windows-installer, macOS + Linux test
├─ ui/                           launcher-frontend (ingen build-step)
│  ├─ index.html
│  ├─ styles.css
│  ├─ main.js                    grid, søg, tilføj/fjern, favoritter
│  └─ frame.{html,css,js}        telefonramme + værktøjslinje i app-vinduet
└─ src-tauri/
   ├─ Cargo.toml
   ├─ tauri.conf.json
   ├─ capabilities/launcher.json rettigheder kun for launcher-vinduet
   ├─ icons/
   └─ src/
      ├─ main.rs
      ├─ lib.rs                  opstart, kommandoer
      ├─ apps.rs                 læs + valider apps.json (frisk ved hver brug)
      ├─ manifest.rs             manifest, ikonvalg, cache
      ├─ store.rs                favoritter, senest åbnet, vinduesplacering
      ├─ secrets.rs              bypass-tokens i keychain
      └─ appwin/
         ├─ mod.rs               mockup-vindue, data-mappe, link-politik, deployment
         ├─ device.rs            enheder + layout-matematik (skala, ramme, skærm)
         ├─ actions.rs           værktøjslinjens handlinger
         └─ status.rs            SW-/offline-status
```

## Hvor ligger data?

| Hvad | Windows | macOS |
|---|---|---|
| Web-data pr. app | `%APPDATA%\dk.thyxo.pwalauncher\webdata\<app-id>\` | WebKit data store pr. app |
| Din app-liste | `%APPDATA%\dk.thyxo.pwalauncher\apps.json` | `~/Library/Application Support/dk.thyxo.pwalauncher/apps.json` |
| Favoritter, vinduer | `%APPDATA%\dk.thyxo.pwalauncher\state.json` | `~/Library/Application Support/dk.thyxo.pwalauncher/state.json` |
| Bypass-tokens | Credential Manager | Keychain |
| Ikon-cache | `%LOCALAPPDATA%\dk.thyxo.pwalauncher\meta\` | `~/Library/Caches/dk.thyxo.pwalauncher/meta/` |
