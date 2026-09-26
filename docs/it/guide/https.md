# HTTPS automatico

OtterRoute può servire **HTTPS da solo**: ottiene un certificato gratuito per ogni dominio verificato, lo rinnova prima della scadenza e lo usa per rispondere sulla porta 443. Non serve un proxy davanti. Se invece hai già un proxy o una CDN che gestisce TLS, vedi [HTTPS e proxy](./https-proxy).

## Come funziona

1. Nelle **Impostazioni → HTTPS automatico** spunti *Ottieni e rinnova i certificati* (e, facoltativo, indichi un'email di contatto per la CA).
2. Per ogni dominio **verificato** il nodo chiede un certificato a Let's Encrypt con la sfida **HTTP-01**: la CA fa una richiesta a `http://tuodominio/.well-known/acme-challenge/…` sulla porta 80 e il nodo risponde con la prova.
3. Il certificato viene salvato in `certs/<dominio>/` nella cartella di stato (la chiave con permessi `0600`) e usato subito: il nodo sceglie il certificato giusto in base al nome richiesto dal browser (SNI).
4. **30 giorni prima della scadenza** il nodo lo rinnova da solo. Se un'emissione fallisce, riprova dopo un'ora (le CA limitano i tentativi) e ti avvisa nella campanella e, se le hai attivate, nelle [notifiche](./notifications).

Lo stato di ogni dominio è nella colonna **HTTPS** della pagina *Domini*: *Valido*, *In scadenza* (meno di 14 giorni), *Scaduto*, *Non emesso*, *In emissione*, *Errore* (con il messaggio della CA). Dalla riga espansa puoi **Richiedere** o **Rinnovare ora** un certificato.

## Cosa serve

- Il dominio deve essere **verificato** e arrivare a questo nodo sulla **porta 80 da Internet**: la CA deve poter raggiungere la sfida. Un firewall che blocca la 80, o un proxy/CDN che non inoltra il percorso `/.well-known/acme-challenge/`, fa fallire l'emissione.
- Il nodo deve poter **ascoltare sulla 443** (o sulla porta di `OTR_HTTPS_LISTEN`). Se la porta non è disponibile il nodo parte lo stesso, senza HTTPS, e lo scrive nel log e in Impostazioni.
- Nomi pubblici: i domini `.localhost` e gli indirizzi IP non possono avere un certificato pubblico.
- **Niente jolly** (`*.example.com`): con HTTP-01 ogni nome ha il suo certificato. Per un certificato jolly ottienilo altrove e caricalo (vedi sotto).

::: warning Dietro Cloudflare o un altro proxy
Se il dominio punta a un proxy (per esempio Cloudflare con la nuvola arancione), la sfida arriva al proxy e non al nodo: l'emissione fallisce. In quel caso lascia i certificati al proxy e usa [HTTPS e proxy](./https-proxy).
:::

## Provare senza limiti: l'ambiente di staging

Let's Encrypt ha limiti di richieste. Spunta *Usa l'ambiente di prova (staging)* per provare tutto il percorso senza consumarli: i certificati di staging **non sono validi nei browser**. Per passare ai certificati veri togli la spunta: il nodo ne richiede di nuovi (l'account presso la CA è separato per ogni ambiente).

## Reindirizzare l'HTTP a HTTPS

Nella riga di un dominio, dopo aver ottenuto il certificato, puoi spuntare *Reindirizza l'HTTP a HTTPS*: il nodo risponde `308` verso `https://…` conservando percorso e query. Non è attivabile finché il dominio non ha un certificato, perché il sito diventerebbe irraggiungibile.

Restano sempre in HTTP: la **verifica del dominio** (`/.well-known/otterroute/check`) e le **sfide ACME**, così la verifica e il rinnovo continuano a funzionare.

## Caricare un certificato tuo

Per una CA aziendale, un certificato jolly o un certificato già acquistato, nella riga espansa del dominio scegli **Carica un certificato**: incolla la catena (certificato + intermedi) e la chiave privata in PEM. Il certificato si usa subito, ma **non si rinnova da solo**: alla scadenza va ricaricato (lo vedi nello stato del dominio e negli avvisi).

## Porte e impostazioni

| | Default | Come cambiarla |
|---|---|---|
| Ascolto HTTPS | `0.0.0.0:443` | `OTR_HTTPS_LISTEN` (vuoto = HTTPS disattivato) |
| Porta HTTPS pubblica (per i redirect) | 443 | Impostazioni → Porte |

L'immagine Docker espone la 443: mappala con `-p 443:443` (in `compose.yaml` è già presente).

## Sicurezza

- Le chiavi private stanno in `certs/<dominio>/privkey.pem` (permessi `0600`); la chiave dell'account ACME in `secrets/`. Vedi [Sicurezza](./security) e [Aggiornamenti e backup](./upgrades-backup): includili nel backup, oppure lasciali rigenerare.
- Il nodo accetta solo TLS 1.2 e 1.3, con ALPN `h2` e `http/1.1`.
- Un handshake senza certificato per il nome richiesto viene rifiutato: nessun certificato "di ripiego" viene mostrato.

## Limiti

- Solo la sfida HTTP-01: niente certificati jolly emessi dal nodo, e serve la porta 80 raggiungibile.
- Un solo nodo per dominio: con più nodi dietro lo stesso nome la sfida può arrivare a un nodo diverso da quello che l'ha richiesta.
- Non c'è la revoca dal pannello; per revocare usa gli strumenti della CA.
- Il flusso con Let's Encrypt è coperto da test contro un server ACME di prova (Pebble) nella CI, non da prove su domini reali in produzione.
