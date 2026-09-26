# Risoluzione dei problemi

Cerca il messaggio che vedi nel pannello o nella risposta. Se non lo trovi, la pagina [Diagnosi](./diagnosis) segue il percorso di un file e dice dove si ferma.

## Domini

| Messaggio / stato | Causa | Cosa fare |
|---|---|---|
| **Errore DNS** — *Nessun record A/AAAA trovato per …* | Il dominio non ha record verso il nodo, o non si è ancora propagato. | Crea il record ([Domini e DNS](./domains-dns)); attendi fino al TTL (talvolta 48 ore); il pannello ricontrolla da solo ogni 5 minuti. |
| **Nodo non raggiungibile** | Il dominio risolve, ma sulla porta 80 dell'indirizzo non risponde nessuno. | Verifica firewall, port forwarding e che il nodo ascolti sulla porta 80 (`OTR_LISTEN`). |
| **Nodo non raggiungibile** — *Su … risponde un altro server/servizio* | Sull'indirizzo c'è un altro server (un proxy, una CDN, il sito vecchio). | Punta il record al nodo giusto, o lascia passare il percorso `/.well-known/otterroute/check` fino al nodo. |
| Verificato, ma dal browser non si apre | Il tuo computer risolve il nome diversamente (cache DNS, `/etc/hosts`) o usi HTTPS. | `dig +short tuodominio`; prova `curl -H 'Host: …' http://IP/`. HTTPS non è servito dal nodo: [HTTPS e proxy](./https-proxy). |
| Il dominio `.local` non si apre | `.local` è riservato al mDNS. | Usa `.localhost` o una riga in `/etc/hosts`. |

## Bucket

| Messaggio | Causa | Cosa fare |
|---|---|---|
| *Lo storage ha rifiutato le credenziali o la richiesta (…)* | Chiave errata, permessi insufficienti, regione o endpoint sbagliati. | Ricontrolla chiavi, regione e stile d'indirizzamento ([Bucket S3](./buckets-s3)); cerca il codice (`SignatureDoesNotMatch`, `AccessDenied`, `AuthorizationHeaderMalformed`…). |
| *Storage non raggiungibile: …* | Rete, DNS dell'endpoint, TLS, porta o firewall. | Prova dal nodo: `curl -I https://ENDPOINT`. |
| Endpoint rifiutato (indirizzo privato) | L'endpoint è `localhost` o su una rete privata. | Spunta *"Lo storage è in rete locale"* se è voluto ([Sicurezza](./security)). |
| Il bucket va in errore dopo un po' | Chiave scaduta, revocata, o storage fermo. | Controlla la chiave; il pannello ricontrolla i bucket periodicamente e segnala l'errore nell'icona degli avvisi. |

## Instradamenti e file

| Sintomo | Causa | Cosa fare |
|---|---|---|
| `404` su un file che esiste | Prefisso o cartella sbagliati; il prefisso del percorso viene tolto prima della ricerca. | Usa la prova nel pannello ([Instradamenti](./routes)); controlla cartella e nome. |
| `404` da un dominio | Nessun instradamento per quell'host, o il prefisso non corrisponde. | Crea l'instradamento; vince il prefisso più lungo. |
| Un file corretto risulta ancora `404` | Cache negativa (60 secondi). | Attendi un minuto. |
| Il file non si aggiorna | La copia è ancora fresca (1 ora). | Attendi, o incrementa `cache_generation` ([Cache](./cache)). |
| `403` con `?exp=&sig=` nell'indirizzo | L'instradamento richiede [link firmati](./signed-links) e il link è scaduto, alterato o emesso con una chiave poi ruotata. | Crea un nuovo link; la pagina [Diagnosi](./diagnosis) dice quale dei tre casi è. |
| `400` con `?w=` o `?fmt=` | Un parametro delle [immagini al volo](./images) non è valido (`w`/`h` 1–4096, `q` 30–95, `fmt` webp/jpeg/png/auto). | Correggi il valore. |
| `422` su un'immagine | Il file non è un'immagine valida, è danneggiato o supera i limiti (64 milioni di pixel). | Controlla l'originale nello storage. |
| `405` / `400` | Il gateway accetta solo `GET` e `HEAD` (405); percorsi con `..` o caratteri di controllo sono rifiutati (400). | Usa un percorso normale. |
| `X-Cache: STALE` | Lo storage non risponde e si serve una copia scaduta. | Ripristina lo storage. |

## Pannello e accesso

| Messaggio | Causa | Cosa fare |
|---|---|---|
| *permesso negato: serve lo scope …* | Il tuo ruolo non include l'azione. | Chiedi a un amministratore ([Utenti](./users-2fa)). |
| *troppi tentativi: riprova tra N minuti* | 5 errori in 5 minuti. | Attendi. |
| *codice errato* | Codice 2FA sbagliato o già usato, orologio sfasato. | Sincronizza l'ora del telefono; attendi il codice successivo o usa un codice di recupero. |
| Perso password/2FA (ultimo amministratore) | — | `otterroute --reset-user NOME` sul nodo. |
| Il pannello non si apre | La porta 9090 ascolta solo su `127.0.0.1`. | Usa un tunnel SSH: `ssh -L 9090:127.0.0.1:9090 server`, oppure [servilo in HTTPS](./panel-https). |
| Il pannello in HTTPS reindirizza all'infinito | Cloudflare è in SSL **Flexible**: parla al nodo in HTTP. | Passa a **Full**. |
| Il pannello in HTTPS non si apre (errore TLS o 525/521) | Il certificato del dominio è scaduto o manca, oppure la 443 non è raggiungibile. | Ripristina dal tunnel locale (Domini → HTTPS) e controlla il firewall. |
| Login richiesto di nuovo dopo il riavvio | Le sessioni sono in memoria. | Normale. |

## Notifiche

| Sintomo | Cosa fare |
|---|---|
| Invia prova: *SMTP: …* | Controlla server, porta e sicurezza (STARTTLS 587, TLS 465); utente e password; a volte serve una password per app ([Notifiche](./notifications#email-smtp)). |
| Invia prova: *chat not found* / *Unauthorized* | Chat id sbagliato, il bot non ha mai ricevuto un messaggio da quella chat, o token errato ([Notifiche](./notifications#telegram)). |
| Non arriva nulla dopo un guasto | Il problema deve durare oltre l'attesa (default 10 minuti) e superare la soglia del canale. |

## Certificati HTTPS

| Sintomo | Cosa fare |
|---|---|
| Certificato in errore: *la CA non ha convalidato il dominio* | Il dominio deve arrivare a questo nodo sulla **porta 80 da Internet**; un firewall o un proxy la bloccano; con Cloudflare la sfida passa solo se non viene reindirizzata a HTTPS (*Always Use HTTPS*) o se metti il record in solo DNS durante l'emissione ([HTTPS automatico](./https)). |
| Il browser dice «certificato non valido» ma il dominio è *Valido* | È attivo l'ambiente di **staging**: toglilo in Impostazioni → HTTPS. |
| Il sito non si apre su HTTPS ma il certificato c'è | Il nodo non è in ascolto sulla 443 (porta occupata o permessi): guarda Impostazioni e il log; con Docker mappa `-p 443:443`. |
| Dopo aver attivato il redirect il sito non si apre | Un proxy davanti reindirizza a sua volta in un ciclo: disattiva il redirect di uno dei due. |

## Avvio

| Sintomo | Cosa fare |
|---|---|
| *permission denied* sulla porta 80 | Esegui come root, usa `setcap 'cap_net_bind_service=+ep'`, o cambia `OTR_LISTEN`. |
| *address already in use* | Un altro processo usa la porta: `ss -ltnp \| grep :80`. |
| Il nodo parte ma il pannello è vuoto | Manca `OTR_UI_DIR` (default `./web/dist`): costruisci il pannello o usa l'immagine Docker. |

## Raccogliere informazioni

```sh
RUST_LOG=otterroute=debug otterroute
curl -s  http://127.0.0.1:9090/healthz
curl -s  http://127.0.0.1:9090/metrics | head
```

Guarda anche gli **avvisi** (campanella) e il **registro attività**.
