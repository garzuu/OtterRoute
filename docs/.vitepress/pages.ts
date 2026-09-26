/**
 * Elenco unico delle pagine della guida: da qui si costruiscono le barre
 * laterali di entrambe le lingue e lo script di parità IT/EN.
 * Il file di ogni pagina è `docs/it/<slug>.md` e `docs/en/<slug>.md`.
 */
export interface Page {
  slug: string;
  it: string;
  en: string;
}

export interface Section {
  it: string;
  en: string;
  pages: Page[];
}

export const sections: Section[] = [
  {
    it: "Per iniziare",
    en: "Getting started",
    pages: [
      { slug: "guide/how-it-works", it: "Come funziona", en: "How it works" },
      { slug: "guide/install", it: "Installazione", en: "Installation" },
      { slug: "guide/first-run", it: "Primo avvio", en: "First run" },
      { slug: "guide/concepts", it: "Concetti", en: "Concepts" },
    ],
  },
  {
    it: "Configurazione",
    en: "Configuration",
    pages: [
      { slug: "guide/domains-dns", it: "Domini e DNS", en: "Domains and DNS" },
      { slug: "guide/buckets-s3", it: "Bucket S3", en: "S3 buckets" },
      { slug: "guide/routes", it: "Instradamenti", en: "Routes" },
      { slug: "guide/images", it: "Immagini al volo", en: "Images on the fly" },
      { slug: "guide/signed-links", it: "Link firmati", en: "Signed links" },
      { slug: "guide/cache", it: "Cache", en: "Cache" },
    ],
  },
  {
    it: "Gestione",
    en: "Operations",
    pages: [
      { slug: "guide/users-2fa", it: "Utenti, permessi e 2FA", en: "Users, permissions and 2FA" },
      { slug: "guide/statistics", it: "Statistiche e metriche", en: "Statistics and metrics" },
      { slug: "guide/diagnosis", it: "Diagnosi", en: "Diagnosis" },
      { slug: "guide/notifications", it: "Notifiche", en: "Notifications" },
      { slug: "guide/https", it: "HTTPS automatico", en: "Automatic HTTPS" },
      { slug: "guide/https-proxy", it: "HTTPS e proxy", en: "HTTPS and proxies" },
      { slug: "guide/security", it: "Sicurezza", en: "Security" },
      { slug: "guide/upgrades-backup", it: "Aggiornamenti e backup", en: "Upgrades and backup" },
    ],
  },
  {
    it: "Aiuto",
    en: "Help",
    pages: [
      { slug: "guide/troubleshooting", it: "Risoluzione dei problemi", en: "Troubleshooting" },
      { slug: "guide/faq", it: "Domande frequenti", en: "FAQ" },
      { slug: "guide/glossary", it: "Glossario", en: "Glossary" },
    ],
  },
  {
    it: "Provider",
    en: "Providers",
    pages: [{ slug: "providers/index", it: "Panoramica e compatibilità", en: "Overview and compatibility" }],
  },
  {
    it: "Provider DNS",
    en: "DNS providers",
    pages: [
      { slug: "providers/dns/cloudflare", it: "Cloudflare", en: "Cloudflare" },
      { slug: "providers/dns/route53", it: "AWS Route 53", en: "AWS Route 53" },
      { slug: "providers/dns/google-cloud-dns", it: "Google Cloud DNS", en: "Google Cloud DNS" },
      { slug: "providers/dns/azure-dns", it: "Azure DNS", en: "Azure DNS" },
      { slug: "providers/dns/ovhcloud", it: "OVHcloud", en: "OVHcloud" },
      { slug: "providers/dns/aruba", it: "Aruba", en: "Aruba" },
    ],
  },
  {
    it: "Provider S3",
    en: "S3 providers",
    pages: [
      { slug: "providers/s3/aws-s3", it: "AWS S3", en: "AWS S3" },
      { slug: "providers/s3/cloudflare-r2", it: "Cloudflare R2", en: "Cloudflare R2" },
      { slug: "providers/s3/backblaze-b2", it: "Backblaze B2", en: "Backblaze B2" },
      { slug: "providers/s3/wasabi", it: "Wasabi", en: "Wasabi" },
      { slug: "providers/s3/digitalocean-spaces", it: "DigitalOcean Spaces", en: "DigitalOcean Spaces" },
      { slug: "providers/s3/hetzner", it: "Hetzner Object Storage", en: "Hetzner Object Storage" },
      { slug: "providers/s3/minio-garage", it: "MinIO e Garage (self-hosted)", en: "MinIO and Garage (self-hosted)" },
    ],
  },
  {
    it: "Riferimento",
    en: "Reference",
    pages: [
      { slug: "reference/config", it: "config.yaml", en: "config.yaml" },
      { slug: "reference/environment", it: "Variabili d'ambiente e flag", en: "Environment variables and flags" },
      { slug: "reference/api", it: "API del pannello", en: "Panel API" },
      { slug: "reference/scopes", it: "Scope e ruoli", en: "Scopes and roles" },
      { slug: "reference/metrics", it: "Metriche Prometheus", en: "Prometheus metrics" },
    ],
  },
];

export const allSlugs = sections.flatMap((s) => s.pages.map((p) => p.slug));
export const allPages = sections.flatMap((s) => s.pages);
