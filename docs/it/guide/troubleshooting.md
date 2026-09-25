# Risoluzione dei problemi

Cerca il messaggio che vedi nel pannello o nella risposta.

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
| `405` / `400` | Il gateway accetta solo `GET` e `HEAD` (405); percorsi con `..` o caratteri di controllo sono rifiutati (400). | Usa un percorso normale. |
| `X-Cache: STALE` | Lo storage non risponde e si serve una copia scaduta. | Ripristina lo storage. |

## Pannello e accesso

| Messaggio | Causa | Cosa fare |
|---|---|---|
| *permesso negato: serve lo scope …* | Il tuo ruolo non include l'azione. | Chiedi a un amministratore ([Utenti](./users-2fa)). |
| *troppi tentativi: riprova tra N minuti* | 5 errori in 5 minuti. | Attendi. |
| *codice errato* | Codice 2FA sbagliato o già usato, orologio sfasato. | Sincronizza l'ora del telefono; attendi il codice successivo o usa un codice di recupero. |
| Perso password/2FA (ultimo amministratore) | — | `otterroute --reset-user NOME` sul nodo. |
| Il pannello non si apre | La porta 9090 ascolta solo su `127.0.0.1`. | Usa un tunnel SSH: `ssh -L 9090:127.0.0.1:9090 server`. |
| Login richiesto di nuovo dopo il riavvio | Le sessioni sono in memoria. | Normale. |

## Notifiche

| Sintomo | Cosa fare |
|---|---|
| Invia prova: *SMTP: …* | Controlla server, porta e sicurezza (STARTTLS 587, TLS 465); utente e password; a volte serve una password per app ([Notifiche](./notifications#email-smtp)). |
| Invia prova: *chat not found* / *Unauthorized* | Chat id sbagliato, il bot non ha mai ricevuto un messaggio da quella chat, o token errato ([Notifiche](./notifications#telegram)). |
| Non arriva nulla dopo un guasto | Il problema deve durare oltre l'attesa (default 10 minuti) e superare la soglia del canale. |

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
