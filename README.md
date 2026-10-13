# PWA Launcher

Native desktop-app (Rust + Tauri 2) til at åbne og teste mine webapps/PWAs som
"rigtige" apps – ét vindue pr. app, egen vedvarende storage, testværktøjer.

WebView2 på Windows, WebKit på macOS/Linux. Ingen Electron.

## Status

| Trin | Indhold | Status |
|---|---|---|
| a | Ét vindue med hardcoded URL (Stopwatch+), egen data-mappe, eksterne links i browser | ✅ |
| b | Vercel-token i keychain + projektopdagelse | ⏳ |
| c | Launcher-grid: manifest, ikoner, søg, sortering, favoritter, manuel URL | ⏳ |
| d | Testmenu: DevTools, viewports, hard reload, ryd data, prod/preview, SW-status | ⏳ |

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

## Vercel-token (trin b)

1. vercel.com → avatar → **Account Settings → Tokens**
2. **Create Token**: navn fx `pwa-launcher`, scope = dit team, vælg udløbsdato.
   Vælg read-only, hvis din konto tilbyder det – launcheren laver kun `GET`-kald.
3. Indsæt tokenet i launcherens **Indstillinger**. Det gemmes i OS'ets keychain
   (Windows Credential Manager / macOS Keychain / Secret Service), aldrig i en fil.

## Arkitektur

```
Launcher-vindue (ui/, vanilla JS)
        │ invoke()
Rust-kerne (src-tauri/src)
  vercel · manifest · store · secrets
        │ WebviewWindowBuilder
App-vinduer (ét pr. app)
  egen data-mappe · testmenu · status i titel
```

| Valg | Løsning |
|---|---|
| Storage pr. app | `data_directory` (Windows/Linux), `data_store_identifier` (macOS 14+) |
| Eksterne links | `on_navigation` / `on_new_window` → standardbrowser |
| Redirects | Indtil første side er indlæst, følges redirects (fx til eget domæne eller Vercel-login) |
| Domæne | Altid `*.vercel.app` |
| Test-UI | Native menulinje "Test" + status i vinduestitel |
| Preview-bypass | `?x-vercel-protection-bypass=…&x-vercel-set-bypass-cookie=true` på første URL |

## Filstruktur

```
AppTester/
├─ README.md
├─ .github/workflows/build.yml   CI: Windows-installer, macOS + Linux test
├─ ui/                           launcher-frontend (ingen build-step)
│  └─ index.html
└─ src-tauri/
   ├─ Cargo.toml
   ├─ tauri.conf.json
   ├─ capabilities/launcher.json rettigheder kun for launcher-vinduet
   ├─ icons/
   └─ src/
      ├─ main.rs
      ├─ lib.rs                  opstart, plugins
      └─ appwin/mod.rs           app-vinduer, data-mappe, link-politik
```

Planlagt i senere trin: `error.rs`, `store.rs`, `secrets.rs`, `vercel.rs`,
`manifest.rs`, `commands.rs`, `appwin/{menu,viewport,status}.rs`,
`ui/{main,settings,api}.js`, `ui/styles.css`.

## Hvor ligger data?

| Hvad | Windows | macOS |
|---|---|---|
| Web-data pr. app | `%APPDATA%\dk.thyxo.pwalauncher\webdata\<app-id>\` | WebKit data store pr. app |
| Token | Credential Manager | Keychain |
