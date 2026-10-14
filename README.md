# PWA Launcher

Native desktop-app (Rust + Tauri 2) til at åbne og teste mine webapps/PWAs som
"rigtige" apps – ét vindue pr. app, egen vedvarende storage, testværktøjer.

WebView2 på Windows, WebKit på macOS/Linux. Ingen Electron.

## Status

| Trin | Indhold | Status |
|---|---|---|
| a | App-vindue med egen data-mappe, eksterne links i browser | ✅ |
| b | App-liste i `apps.json` + launcher-vindue | ✅ |
| c | Manifest-ikoner/farver, søg, sortering, favoritter, husk vinduesstørrelse | ⏳ |
| d | Testmenu: DevTools, viewports, hard reload, ryd data, SW-/offline-status | ⏳ |

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

Apps står i [`apps.json`](apps.json) i repo-roden. Der er ingen automatisk opdagelse –
du tilføjer selv dem, du vil have:

```json
[
  { "id": "stopwatchplus", "name": "Stopwatch+", "url": "https://stopwatchplus.vercel.app/" },
  { "id": "calcplus",      "name": "Calc+",      "url": "https://calcplus.vercel.app/" }
]
```

- `id`: små bogstaver, tal, `-` og `_`. Appens storage følger id'et – omdøb det ikke,
  hvis du vil beholde dens data.
- Filen bygges ind i programmet: genstart `cargo tauri dev` (eller byg igen) efter ændringer.
- Fejl i filen (dublet-id, forkert URL, stavefejl i et felt) vises i launcheren,
  og `cargo test` fanger dem også.

## Arkitektur

```
Launcher-vindue (ui/, vanilla JS)
        │ invoke()
Rust-kerne (src-tauri/src)
  apps.json · appwin
        │ WebviewWindowBuilder
App-vinduer (ét pr. app)
  egen data-mappe · testmenu · status i titel
```

| Valg | Løsning |
|---|---|
| Storage pr. app | `data_directory` (Windows/Linux), `data_store_identifier` (macOS 14+) |
| Eksterne links | `on_navigation` / `on_new_window` → standardbrowser |
| Redirects | Indtil første side er indlæst, følges redirects (fx til eget domæne eller Vercel-login) |
| Test-UI | Native menulinje "Test" + status i vinduestitel |
| Rettigheder | Kun launcher-vinduet må kalde `list_apps`/`open_app`; webapps i app-vinduer har ingen IPC-adgang |

## Filstruktur

```
AppTester/
├─ README.md
├─ apps.json                     DIN app-liste
├─ .github/workflows/build.yml   CI: Windows-installer, macOS + Linux test
├─ ui/                           launcher-frontend (ingen build-step)
│  ├─ index.html
│  ├─ styles.css
│  └─ main.js                    grid, åbn app
└─ src-tauri/
   ├─ Cargo.toml
   ├─ tauri.conf.json
   ├─ capabilities/launcher.json rettigheder kun for launcher-vinduet
   ├─ icons/
   └─ src/
      ├─ main.rs
      ├─ lib.rs                  opstart, kommandoer
      ├─ apps.rs                 indlæs + valider apps.json
      └─ appwin/mod.rs           app-vinduer, data-mappe, link-politik
```

Planlagt: `manifest.rs`, `store.rs` (favoritter, vinduesstørrelser),
`appwin/{menu,viewport,status}.rs`.

## Hvor ligger data?

| Hvad | Windows | macOS |
|---|---|---|
| Web-data pr. app | `%APPDATA%\dk.thyxo.pwalauncher\webdata\<app-id>\` | WebKit data store pr. app |
