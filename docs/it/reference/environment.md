# Variabili d'ambiente e opzioni

Ogni opzione si può dare come flag (`--cache-dir`) o come variabile d'ambiente. Il flag ha la precedenza.

| Variabile | Flag | Default | Descrizione |
|---|---|---|---|
| `OTR_CONFIG` | `--config` | `config.yaml` | File di configurazione. Con il pannello è generato da `panel.json`. |
| `OTR_LISTEN` | `--listen` | `0.0.0.0:80` | Indirizzo pubblico HTTP. |
| `OTR_HTTPS_LISTEN` | `--https-listen` | `0.0.0.0:443` | Indirizzo HTTPS pubblico. Vuoto = HTTPS disattivato. Se la porta non è disponibile il nodo parte lo stesso (senza HTTPS). |
| `OTR_ACME_DIRECTORY` | `--acme-directory` | — | Directory ACME alternativa a Let's Encrypt (per le prove, es. Pebble). |
| `OTR_ACME_CA_ROOT` | `--acme-ca-root` | — | File PEM con la CA radice della directory alternativa. |
| `OTR_UPDATE_CHECK` | `--update-check` | `on` | `off` spegne ogni richiesta verso GitHub per cercare nuove versioni. |
| `OTR_UPDATE_API` | `--update-api` | GitHub (release ufficiali) | API delle release per fork o mirror interni. |
| `OTR_UPDATE_KEY` | `--update-key` | chiave ufficiale | Chiave pubblica Ed25519 (esadecimale, 32 byte) con cui si verificano le release: serve solo a fork o mirror interni che firmano con una chiave propria. |
| `OTR_INSTALL` | `--install` | — | Come è installato il nodo: `docker` (nell'immagine ufficiale), `service`, `binary` o `source`. Decide le istruzioni di aggiornamento e se il nodo può aggiornarsi da solo (`service` e `binary`). |
| `OTR_ADMIN_LISTEN` | `--admin-listen` | `127.0.0.1:9090` | Pannello, API, `/healthz` e `/metrics`. Tienilo in locale. |
| `OTR_CACHE_DIR` | `--cache-dir` | `./data/cache` | Cartella della cache su disco. |
| `OTR_CACHE_MAX_BYTES` | `--cache-max-bytes` | `10737418240` (10 GiB) | Dimensione massima della cache. |
| `OTR_CACHE_MAX_OBJECT_BYTES` | `--cache-max-object-bytes` | `1073741824` (1 GiB) | Oltre questa dimensione un file non viene salvato in cache. |
| `OTR_STATE_DIR` | `--state-dir` | `./data/state` | Utenti, configurazione del pannello, chiavi, statistiche, registro. |
| `OTR_DOMAIN_RECHECK` | `--domain-recheck` | `5m` | Ogni quanto ricontrollare domini e bucket **non** verificati. |
| `OTR_DOMAIN_RECHECK_VERIFIED` | `--domain-recheck-verified` | `15m` | Ogni quanto ricontrollare quelli già verificati. |
| `OTR_UI_DIR` | `--ui-dir` | `./web/dist` | Cartella con il pannello compilato. |
| `OTR_PUBLIC_URL` | `--public-url` | — | Indirizzo con cui si raggiunge il pannello: se impostato, le notifiche contengono il link. |
| `OTR_DOCS_DIR` | `--docs-dir` | — | Cartella con la guida costruita: se esiste, è servita dalla porta del pannello sotto `/docs/` (guida offline). |
| `OTR_DOCS_URL` | `--docs-url` | `https://garzuu.github.io/OtterRoute/` | Indirizzo della guida online usato dai link «Guida» del pannello. Vuoto = nessun link. Ha la precedenza la copia offline, se presente. |
| `OTR_RELOAD_INTERVAL` | `--reload-interval` | `2s` | Ogni quanto rileggere il file di configurazione. |
| — | `--reset-user NOME` | — | Reimposta password e 2FA dell'utente e termina. |
| — | `--check-update` | — | Cerca una versione nuova, la mostra ed esce. |
| — | `--self-update` | — | Scarica, verifica e installa la versione nuova (eseguibile, pannello, guida) ed esce; il riavvio resta a te. Solo per installazioni `binary` e `service`. |
| — | `--self-check` | — | Prova d'avvio con stato e porte temporanei: stampa `ok` ed esce con 0 se il nodo risponde a `/healthz`. |
| — | `--healthcheck` | — | Controlla `/healthz` sul pannello locale ed esce (0 = ok): è l'`HEALTHCHECK` dell'immagine Docker. |

Le durate si scrivono come `30s`, `5m`, `1h`, `24h`, `7d`.

Nell'immagine Docker `OTR_LISTEN`, `OTR_STATE_DIR` e `OTR_CACHE_DIR` sono già impostate sui volumi `/data`.

## Log

Il livello si controlla con `RUST_LOG`, per esempio `RUST_LOG=otterroute=debug`.

## Porte

Le porte pubbliche HTTP/HTTPS mostrate nelle **Impostazioni** del pannello sono quelle usate dalle verifiche e dalle indicazioni per il firewall. L'indirizzo su cui il nodo ascolta davvero resta `OTR_LISTEN`.

| Porta | Uso | Nota |
|---|---|---|
| 80 | Traffico pubblico HTTP | Serve i file e la prova di verifica. |
| 443 | HTTPS | Certificati automatici o caricati. [HTTPS automatico](/guide/https) |
| 9090 | Pannello, API, metriche | Solo `127.0.0.1` per default. |
