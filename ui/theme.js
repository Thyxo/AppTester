// Sætter farvetema så tidligt som muligt (indlæses i <head>), så siden ikke blinker.
// Launcher og mockup-ramme deler localStorage, så valget gælder begge.
(function () {
  const THEMES = [
    { id: "nat", name: "Nat", swatch: ["#131118", "#aac8f3", "#ff8b80", "#c13ee0", "#d8f7c8"] },
    { id: "lys", name: "Lys", swatch: ["#f3f1f6", "#b9d3f7", "#ffa69d", "#c13ee0", "#d3f5c1"] },
    { id: "hav", name: "Hav", swatch: ["#0c1b2d", "#9fd8ff", "#ffd37a", "#5eead4", "#f9a8d4"] },
    { id: "grafit", name: "Grafit", swatch: ["#0f0f10", "#e4e4e7", "#a1a1aa", "#d9f99d", "#3f3f46"] },
  ];
  const valid = (id) => THEMES.some((t) => t.id === id);

  let current = "nat";
  try {
    const saved = localStorage.getItem("theme");
    if (valid(saved)) current = saved;
  } catch {
    /* Uden storage bruges standardtemaet. */
  }
  document.documentElement.dataset.theme = current;

  window.PWA_THEMES = THEMES;
  window.getTheme = () => document.documentElement.dataset.theme;
  window.setTheme = (id) => {
    if (!valid(id)) return;
    document.documentElement.dataset.theme = id;
    try {
      localStorage.setItem("theme", id);
    } catch {
      /* ikke vigtigt */
    }
    window.dispatchEvent(new CustomEvent("themechange", { detail: id }));
  };

  // Skift i et andet vindue (fx launcheren) slår igennem her.
  window.addEventListener("storage", (e) => {
    if (e.key === "theme" && valid(e.newValue)) {
      document.documentElement.dataset.theme = e.newValue;
      window.dispatchEvent(new CustomEvent("themechange", { detail: e.newValue }));
    }
  });
})();
