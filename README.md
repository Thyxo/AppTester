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

- **Temaer**: Nat (standard), Lys, Hav og Grafit – skift med palet-knappen eller under
  **⚙ Indstillinger**. Valget gælder også mockup-rammen.
- **Fliserne** øverst filtrerer: Alle, Favoritter, Senest åbnet – og **+ Tilføj app**.
- **Ikoner og farver** hentes fra appens web manifest og caches, så det hele ser ens
  ud offline. **↻** henter igen.
- **Søg**: `/` eller `Ctrl+F`, `Enter` åbner første app, `Esc` rydder.
- **☆** gør en app til favorit (vises først), **✕** fjerner den fra listen.

## App-vinduet: enheds-mockup

Hver app åbner i en realistisk enhed (iPhone 15 Pro som standard) uden Windows-ramme:
titaniumkant, sideknapper, Dynamic Island, statuslinje med klokkeslæt og
hjem-indikator. Statuslinjen og bunden farves med appens `theme_color` og
`background_color`, ligesom når appen er installeret på telefonen.

- **Forholdet er låst** og vinduet kan ikke blive større end 100 %. Gør du det mindre
  (grebet nederst til højre), skaleres hele enheden – appen ser stadig samme viewport.
- **Flyt** vinduet ved at trække i værktøjslinjen eller kabinettet.
- Husker størrelse, placering og enhed pr. app.

| Enhed | Skærm | Appens viewport (portræt) |
|---|---|---|
| iPhone 15 Pro | 393×852 | 393×759 (minus statuslinje 59 og hjem-indikator 34) |
| iPhone 15 Pro Max | 430×932 | 430×839 |
| iPhone SE | 375×667 | 375×647 (minus statuslinje 20) |
| Pixel 8 | 412×915 | 412×867 (minus statuslinje og navigationslinje 24+24) |
| iPad Air | 820×1180 | 820×1136 |
| Desktop | 1280×800 | 1280×800 |

Viewporten svarer til en installeret PWA (standalone). Holder du musen over
enhedsvælgeren, står den aktuelle viewport. User-agent og touch ændres ikke.

**Værktøjslinjen**

| Knap | Hvad det gør |
|---|---|
| PROD / PREVIEW | Skift mellem production og preview (hvis appen har en preview-adresse) |
| SW ✓ · Online | Status: service worker styrer siden / netværk (offline – kører fra cache) |
| ⟳ | Genindlæs |
| ⚡ | Hard reload: afregistrerer service workers og tømmer Cache Storage |
| 🗑 | Ryd alle data for appen (med bekræftelse) |
| `<>` | DevTools for appen |
| Enhed | Vælg enhed |
| ⟲ | Vend (portræt/landskab) |
| ↗ | Åbn i standardbrowser |

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
│  ├─ frame.{html,css,js}        enheds-mockup + værktøjslinje i app-vinduet
│  ├─ theme.{css,js}             farvetemaer (deles af launcher og mockup)
│  └─ fonts/                     Poppins (SIL Open Font License)
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
