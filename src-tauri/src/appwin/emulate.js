// Kører i appens webview før sidens egne scripts (initialization script).
// Får appen til at opføre sig som en installeret PWA på telefonen.
// Pladsholdere udfyldes i emulate.rs.
(function () {
  "use strict";
  if (window.__pwaLauncher) return;
  var FONT = "__FONT__"; // base64 woff2 eller "" (macOS har selv SF Pro)
  // { statusBar: null | "island" | "classic" | "tablet", screen: null | [bredde, højde] }
  var config = __CONFIG__;

  // ---------- Skrift: iPhonens systemskrift → Inter ----------
  // SF Pro findes ikke på Windows. Inter (variabel, 100–900) ligner den og har
  // de tynde vægte, som fx iOS-ure bruger.
  if (FONT) {
    try {
      var bin = atob(FONT);
      var bytes = new Uint8Array(bin.length);
      for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      [
        "-apple-system", "BlinkMacSystemFont", "system-ui", "ui-sans-serif",
        "SF Pro", "SF Pro Text", "SF Pro Display", "SF UI Text", "SF UI Display",
        ".SFNSText", ".SFNSDisplay",
      ].forEach(function (family) {
        var face = new FontFace(family, bytes.buffer, { weight: "100 900", style: "normal" });
        document.fonts.add(face);
        face.load().catch(function () {});
      });
    } catch (e) {}
  }

  // ---------- Installeret PWA ----------
  var ios = function () {
    var ua = navigator.userAgent;
    return /iPhone|iPad/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1);
  };
  var mobile = function () {
    return /iPhone|iPad|Android/.test(navigator.userAgent) || ios();
  };
  try {
    Object.defineProperty(Navigator.prototype, "standalone", {
      configurable: true,
      get: function () { return ios() ? true : undefined; },
    });
  } catch (e) {}
  try {
    // Safari har ikke userAgentData; Chromium afslører ellers Windows.
    var uad = Object.getOwnPropertyDescriptor(Navigator.prototype, "userAgentData");
    if (uad && uad.get) {
      Object.defineProperty(Navigator.prototype, "userAgentData", {
        configurable: true,
        get: function () { return ios() ? undefined : uad.get.call(this); },
      });
    }
  } catch (e) {}
  try {
    // display-mode: standalone (CSS-varianten kan ikke efterlignes, kun matchMedia).
    var mm = window.matchMedia.bind(window);
    var DISPLAY = /\(\s*display-mode\s*:\s*([a-z-]+)\s*\)/gi;
    window.matchMedia = function (query) {
      var q = String(query);
      if (!mobile() || !DISPLAY.test(q)) return mm(q);
      DISPLAY.lastIndex = 0;
      // Erstat display-mode med noget, der altid/aldrig gælder, og bevar resten.
      var rewritten = q.replace(DISPLAY, function (_, mode) {
        return mode.toLowerCase() === "standalone" ? "(min-width: 0px)" : "(max-width: -1px)";
      });
      var res = mm(rewritten);
      try { Object.defineProperty(res, "media", { value: q }); } catch (e) {}
      return res;
    };
  } catch (e) {}

  // ---------- Skærmstørrelse ----------
  // screen.* skal være telefonens, ikke Windows-skærmens (fx bruger iOS-apps
  // screen.height til at rette højden i hjemmeskærm-tilstand).
  try {
    ["width", "height", "availWidth", "availHeight"].forEach(function (prop) {
      var d = Object.getOwnPropertyDescriptor(Screen.prototype, prop);
      if (!d || !d.get) return;
      Object.defineProperty(Screen.prototype, prop, {
        configurable: true,
        get: function () {
          var s = config.screen;
          if (!s) return d.get.call(this);
          return /width/i.test(prop) ? s[0] : s[1];
        },
      });
    });
  } catch (e) {}

  // ---------- iOS-statuslinje oven på appen (fuld skærm) ----------
  var host = null;
  var clock = null;
  var timer = 0;

  var ICONS =
    '<svg viewBox="0 0 18 12" class="sig"><rect x="0" y="8" width="3" height="4" rx="1"/><rect x="5" y="5.5" width="3" height="6.5" rx="1"/><rect x="10" y="3" width="3" height="9" rx="1"/><rect x="15" y="0" width="3" height="12" rx="1"/></svg>' +
    '<svg viewBox="0 0 16 12" class="wifi"><path d="M8 2.3c2.4 0 4.6.9 6.2 2.5l1.2-1.2C13.5 1.7 10.9.6 8 .6S2.5 1.7.6 3.6l1.2 1.2C3.4 3.2 5.6 2.3 8 2.3zm0 3.4c1.5 0 2.8.6 3.8 1.5L13 6c-1.3-1.3-3.1-2-5-2s-3.7.7-5 2l1.2 1.2c1-.9 2.3-1.5 3.8-1.5zm0 3.4c.6 0 1.1.2 1.5.6L8 11.4 6.5 9.7c.4-.4.9-.6 1.5-.6z"/></svg>' +
    '<svg viewBox="0 0 27 13" class="bat"><rect x=".5" y=".5" width="23" height="12" rx="3.5" fill="none" stroke="currentColor" opacity=".4"/><rect x="2" y="2" width="20" height="9" rx="2"/><path d="M25 4.5v4c.8-.3 1.3-1.1 1.3-2s-.5-1.7-1.3-2z" opacity=".4"/></svg>';

  // Mål i punkter (= CSS-px i appen).
  var CSS =
    ":host{all:initial;position:fixed;left:0;right:0;top:0;z-index:2147483647;pointer-events:none;" +
    "color:#fff;font-family:-apple-system,BlinkMacSystemFont,sans-serif;font-weight:600;" +
    "-webkit-font-smoothing:antialiased;display:flex;align-items:center}" +
    ".t{font-variant-numeric:tabular-nums;letter-spacing:-.2px}" +
    ".i{display:flex;align-items:center}" +
    "svg{fill:currentColor;display:block}" +
    // iPhone med Dynamic Island: tid og ikoner centreret i hver sin side af øen.
    ":host(.island){height:59px;font-size:17px}" +
    ":host(.island) .t,:host(.island) .i{flex:1;justify-content:center;text-align:center;padding-top:1px}" +
    ":host(.island) .mid{flex:0 0 125px}" +
    ":host(.island) .sig{height:12px;margin-right:7px}:host(.island) .wifi{height:12px;margin-right:7px}:host(.island) .bat{height:13px}" +
    // iPhone SE: tid i midten, ikoner til højre.
    ":host(.classic){height:20px;font-size:12px;justify-content:center}" +
    ":host(.classic) .i{position:absolute;right:6px;top:0;height:20px}" +
    ":host(.classic) .mid{display:none}" +
    ":host(.classic) .sig{height:8px;margin-right:4px}:host(.classic) .wifi{height:8px;margin-right:4px}:host(.classic) .bat{height:9px}" +
    // iPad: tid til venstre, ikoner til højre.
    ":host(.tablet){height:24px;font-size:12.5px;justify-content:space-between;padding:0 20px}" +
    ":host(.tablet) .mid{display:none}" +
    ":host(.tablet) .sig{height:9px;margin-right:5px}:host(.tablet) .wifi{height:9px;margin-right:5px}:host(.tablet) .bat{height:10px}";

  function time() {
    var d = new Date();
    var sep = /^da|^nb|^fi/i.test(navigator.language || "") ? "." : ":";
    return d.getHours() + sep + String(d.getMinutes()).padStart(2, "0");
  }

  function mount() {
    if (host || !document.documentElement) return;
    host = document.createElement("pwa-launcher-statusbar");
    var root = host.attachShadow({ mode: "closed" });
    root.innerHTML = "<style>" + CSS + '</style><span class="t"></span><span class="mid"></span><span class="i">' + ICONS + "</span>";
    clock = root.querySelector(".t");
    document.documentElement.appendChild(host);
  }

  function statusBar(style) {
    clearInterval(timer);
    if (!style) {
      if (host) host.remove();
      host = clock = null;
      return;
    }
    mount();
    if (!host) return;
    host.className = style;
    clock.textContent = time();
    timer = setInterval(function () { clock.textContent = time(); }, 10000);
    // Sider, der erstatter <html>-indholdet, må ikke fjerne den.
    if (!host.isConnected) document.documentElement.appendChild(host);
  }

  // Rust opdaterer indstillingerne, når enhed, retning eller statuslinje skifter.
  function configure(next) {
    var resized = String(next.screen) !== String(config.screen);
    config = next;
    statusBar(config.statusBar);
    if (resized) {
      try { window.dispatchEvent(new Event("resize")); } catch (e) {}
    }
  }
  window.__pwaLauncher = { configure: configure };

  if (config.statusBar) {
    if (document.documentElement) statusBar(config.statusBar);
    else {
      // Scriptet kan køre, før <html> findes.
      var mo = new MutationObserver(function () {
        if (!document.documentElement) return;
        mo.disconnect();
        statusBar(config.statusBar);
      });
      mo.observe(document, { childList: true });
    }
  }
  // Hold den sidst i dokumentet, hvis siden selv tilføjer elementer til <html>.
  document.addEventListener("DOMContentLoaded", function () {
    if (host && host.isConnected) document.documentElement.appendChild(host);
  });
})();
