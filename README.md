<p align="center">
  <img src="docs/assets/logo.png" alt="OtterRoute" width="360">
</p>

# OtterRoute

OtterRoute è un gateway self-hosted che pubblica file da più bucket S3
compatibili attraverso più domini, con cache su disco. Colleghi uno storage,
associ un dominio e ottieni un URL funzionante, senza toccare la
configurazione di un proxy.

📖 **[Guida completa](https://garzuu.github.io/OtterRoute/)** (italiano) ·
**[English guide](https://garzuu.github.io/OtterRoute/en/)** — come funziona,
installazione, configurazione di DNS e storage per Cloudflare, Route 53,
Google Cloud DNS, Azure DNS, OVHcloud, Aruba, AWS S3, R2, Backblaze B2, Wasabi,
DigitalOcean Spaces, Hetzner, MinIO e Garage, sicurezza, risoluzione dei problemi.
I sorgenti della guida sono in [`docs/`](docs/).

## In breve

- **Instradamento** per host e prefisso di percorso, più bucket con credenziali separate.
- **Immagini al volo** (ridimensionamento e WebP) e **link firmati** con scadenza, per instradamento.
- **Sola lettura** (`GET`/`HEAD`), firma SigV4, cache su disco con una sola richiesta allo storage per oggetto.
- **Pannello** su `127.0.0.1:9090`: domini verificati davvero (DNS + richiesta al nodo), bucket, instradamenti, statistiche, utenti con permessi e 2FA, registro attività.
- **Notifiche** email (SMTP) e Telegram quando un dominio o un bucket va in errore, e quando rientra.
- **Metriche Prometheus** su `/metrics`.
- **HTTP** sulla porta 80 e **HTTPS automatico** (certificati gratuiti che si rinnovano da soli) sulla 443; oppure un proxy davanti (vedi la guida).

## Avvio rapido

```sh
docker build -t otterroute .
docker run -d --name otterroute \
  -p 80:80 -p 443:443 -p 127.0.0.1:9090:9090 \
  -v otterroute-data:/data \
  otterroute
```

Apri `http://127.0.0.1:9090/`, crea l'amministratore e segui la
configurazione guidata. La guida è disponibile anche offline sulla stessa
porta, a `http://127.0.0.1:9090/docs/`.

## Sviluppo

```sh
cargo test --workspace            # include i controlli sulla documentazione
./scripts/local-e2e.sh            # compila, avvia due finti S3 con firma SigV4 e lancia i test end-to-end
./scripts/auth-smoke.sh           # utenti, scope, 2FA e audit contro un'istanza vera
cd web  && npm ci && npm run build   # pannello
cd docs && npm ci && npm run dev     # guida (npm run docs:build: parità IT/EN + link)
```

I test in `crates/gateway/src/docs_check.rs` confrontano la documentazione con
il codice: esempi `config.yaml`, variabili `OTR_*`, scope e metriche. Una
pagina inglese va aggiornata insieme a quella italiana (`source_commit`).

## Stato e roadmap

Prototipo con pannello: motore, pannello, utenti e statistiche funzionano su un
singolo nodo.

| Fase | Risultato verificabile |
|---|---|
| **Prototipo tecnico** | Due domini servono due bucket privati differenti, con cache isolata |
| MVP singolo nodo | HTTPS automatico, controller, pannello, diagnostica, backup e ripristino |
| Versione cluster | Helm, configurazioni sincronizzate, stato di applicazione per replica |
| Estensioni | Link firmati, ottimizzazione immagini, accessi separati per clienti |

## English

OtterRoute is a self-hosted, read-only gateway that publishes files from several
private S3-compatible buckets through several domains, with a disk cache. Run
the Docker command above, open `http://127.0.0.1:9090/`, create the
administrator and follow the guided setup. Full documentation, including DNS and
S3 provider guides: <https://garzuu.github.io/OtterRoute/en/>.

## Licenza

MIT oppure Apache-2.0, a scelta.
