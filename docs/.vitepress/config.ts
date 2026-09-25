import { defineConfig, type DefaultTheme } from "vitepress";
import { sections } from "./pages";

// Indirizzo base: `/OtterRoute/` su GitHub Pages, `/docs/` se servita dal nodo,
// `/` in locale. Si imposta con DOCS_BASE.
const base = process.env.DOCS_BASE ?? "/";

const sidebar = (lang: "it" | "en"): DefaultTheme.SidebarItem[] =>
  sections.map((s) => ({
    text: s[lang],
    collapsed: false,
    items: s.pages.map((p) => ({
      text: p[lang],
      link: `${lang === "en" ? "/en" : ""}/${p.slug.replace(/\/index$/, "/")}`,
    })),
  }));

const t = {
  it: {
    nav: [
      { text: "Guida", link: "/guide/how-it-works" },
      { text: "Provider", link: "/providers/" },
      { text: "Riferimento", link: "/reference/config" },
    ],
    outline: "In questa pagina",
    edit: "Modifica questa pagina",
    prev: "Precedente",
    next: "Successiva",
    updated: "Ultimo aggiornamento",
    search: "Cerca",
    noResults: "Nessun risultato per",
  },
  en: {
    nav: [
      { text: "Guide", link: "/en/guide/how-it-works" },
      { text: "Providers", link: "/en/providers/" },
      { text: "Reference", link: "/en/reference/config" },
    ],
    outline: "On this page",
    edit: "Edit this page",
    prev: "Previous",
    next: "Next",
    updated: "Last updated",
    search: "Search",
    noResults: "No results for",
  },
};

const localeTheme = (lang: "it" | "en"): DefaultTheme.Config => ({
  nav: t[lang].nav,
  sidebar: sidebar(lang),
  outline: { label: t[lang].outline, level: [2, 3] },
  docFooter: { prev: t[lang].prev, next: t[lang].next },
  lastUpdated: { text: t[lang].updated },
  editLink: {
    pattern: "https://github.com/garzuu/OtterRoute/edit/main/docs/:path",
    text: t[lang].edit,
  },
  darkModeSwitchLabel: lang === "it" ? "Tema" : "Theme",
  sidebarMenuLabel: lang === "it" ? "Menu" : "Menu",
  returnToTopLabel: lang === "it" ? "Torna su" : "Back to top",
});

export default defineConfig({
  title: "OtterRoute",
  description: "Gateway self-hosted per pubblicare bucket S3 su più domini, con cache su disco.",
  base,
  srcExclude: ["scripts/**", "README.md"],
  // i file sorgente italiani stanno in it/ ma si servono dalla radice del sito
  rewrites: { "it/:rest*": ":rest*" },
  cleanUrls: true,
  lastUpdated: true,
  head: [["link", { rel: "icon", type: "image/png", href: `${base}favicon.png` }]],
  locales: {
    root: { label: "Italiano", lang: "it", themeConfig: localeTheme("it") },
    en: {
      label: "English",
      lang: "en",
      link: "/en/",
      description: "Self-hosted gateway that publishes S3 buckets on many domains, with a disk cache.",
      themeConfig: localeTheme("en"),
    },
  },
  themeConfig: {
    logo: "/logo.png",
    socialLinks: [{ icon: "github", link: "https://github.com/garzuu/OtterRoute" }],
    search: {
      provider: "local",
      options: {
        locales: {
          root: {
            translations: {
              button: { buttonText: t.it.search, buttonAriaLabel: t.it.search },
              modal: { noResultsText: t.it.noResults, displayDetails: "Mostra i dettagli", footer: { selectText: "seleziona", navigateText: "naviga", closeText: "chiudi" } },
            },
          },
          en: {
            translations: {
              button: { buttonText: t.en.search, buttonAriaLabel: t.en.search },
              modal: { noResultsText: t.en.noResults },
            },
          },
        },
      },
    },
    footer: { message: "Rilasciato con licenza MIT o Apache-2.0, a scelta.", copyright: "OtterRoute" },
  },
});
