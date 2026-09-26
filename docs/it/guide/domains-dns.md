# Domini e DNS

Un dominio è utilizzabile solo dopo essere stato **verificato**. La verifica non si limita a controllare che il nome esista: prova che una richiesta al dominio arrivi *a questo nodo*, qualunque strada faccia.

![Un dominio in attesa: il passaggio DNS fallisce e il secondo viene saltato](/screens/domains.jpg)
<p class="shot-caption">Un dominio in attesa: il passaggio DNS fallisce e il secondo viene saltato · 26/09/2026</p>

## Cosa controlla la verifica

<!--@include: @/diagrams/domain-check.it.svg-->

Per ogni dominio il pannello esegue in sequenza:

| Passaggio | Cosa fa | Se fallisce |
|---|---|---|
| **1. Il dominio risolve** | Interroga i DNS per i record A/AAAA. Usa i server DNS del sistema ma **non** `/etc/hosts`, e chiede il nome come assoluto (con il punto finale) così i domini di ricerca non alterano l'esito. | Nessun record trovato: il record non esiste o non si è ancora propagato. |
| **2. Il nodo risponde** | Fa una richiesta HTTP agli indirizzi trovati, con l'header `Host` del dominio, alla pagina di prova `/.well-known/otterroute/check` e verifica una prova firmata con l'identità del nodo. | Nessuna risposta, oppure risponde un altro server. |

La pagina di prova è servita da ogni nodo per qualsiasi host, prima delle regole. Puoi rifare la stessa richiesta a mano da qualunque macchina:

```sh
curl -s http://media.azienda.it/.well-known/otterroute/check?nonce=prova
# {"otterroute":true,"proof":"…"}
```

::: info Non conta dove sia il nodo
Il nodo può avere un IP pubblico o privato, stare dietro NAT, dietro un proxy, un CDN o un load balancer: se la richiesta fatta al dominio arriva a lui, il dominio è valido. Non serve dire al pannello quale sia il suo IP.
:::

## Quali record creare

Il DNS deve portare il nome del dominio **al tuo punto di ingresso**: l'IP del nodo, oppure quello del proxy/CDN/load balancer che gli sta davanti.

| Situazione | Record |
|---|---|
| Sottodominio → IP fisso | `A` (IPv4) e/o `AAAA` (IPv6) verso l'indirizzo |
| Sottodominio → un altro nome | `CNAME` verso quel nome |
| **Dominio principale** (apex, es. `azienda.it`) | il `CNAME` di solito **non è ammesso**: usa un record `A`/`AAAA`, oppure `ALIAS`/`ANAME`/CNAME flattening se il provider lo offre |
| Dietro a un CDN/proxy (es. Cloudflare) | il record che indica il provider, con il proxy attivo o meno secondo le sue regole |

Ogni provider ha le sue particolarità (limiti sull'apex, proxy, TTL minimi): trovi le schede in [Provider DNS](/providers/#dns).

## Propagazione e tempi

Un record nuovo può richiedere **fino a 48 ore** per essere visibile ovunque, e molto meno di solito. Dipende dal TTL del record e da quanto a lungo i resolver ricordano una risposta negativa. Per questo:

- Un dominio nuovo resta **In attesa**, non in errore: è normale.
- Il nodo **ricontrolla da solo**: ogni **5 minuti** i domini in attesa o in errore, ogni **15** quelli verificati. Non serve restare sulla pagina.
- Puoi forzare un controllo con **Ricontrolla** dalla riga del dominio.

Per vedere cosa risponde un resolver pubblico, senza la cache del tuo computer:

```sh
dig +short A media.azienda.it @1.1.1.1
dig +short CNAME media.azienda.it @8.8.8.8
dig +trace media.azienda.it          # segue la catena fino ai server autoritativi
```

## Gli stati di un dominio

| Stato | Significato | Cosa fare |
|---|---|---|
| **In attesa** | Mai verificato. | Crea il record e aspetta la propagazione. |
| **Verificato** | Risolve e la richiesta arriva al nodo. | Puoi usarlo negli instradamenti. |
| **Errore DNS** | Era valido e ora non risolve più (o non porta più qui). Le regole già create continuano a essere servite. | Controlla il record DNS: scaduto, cambiato o cancellato. |
| **Nodo non raggiungibile** | Risolve, ma la richiesta non arriva al nodo. | Controlla proxy, firewall, port forwarding, porta di ingresso. |

Se un dominio verificato fallisce, il nodo **riprova dopo pochi secondi** prima di dichiarare l'errore, così un timeout isolato non genera falsi allarmi. Gli stati compaiono anche come **avvisi** nella campanella del pannello.

## La porta di ingresso

Il controllo fa una richiesta **HTTP** alla porta standard **80**. Se il tuo instradamento cambia la porta (per esempio un proxy che accetta sulla 80 e inoltra alla 8080) la porta verificata resta la 80; se invece il punto di ingresso è un'altra porta, cambiala in **Impostazioni → HTTP**. La porta su cui il nodo *ascolta* si imposta con `OTR_LISTEN`.

::: warning Redirect da HTTP a HTTPS
Il controllo non segue i redirect. Se il tuo proxy o CDN risponde `301`/`302` verso HTTPS a ogni richiesta HTTP, la verifica riporta "risponde un altro servizio (stato 301)". Lascia passare in HTTP il solo percorso `/.well-known/otterroute/check` (vedi [HTTPS e proxy](./https-proxy)).
:::

## Domini locali per provare

I nomi che finiscono in `.localhost` (`img.localhost`) sono sempre validi: i browser li risolvono da soli su `127.0.0.1` e il pannello li considera verificati senza controlli. Servono solo per provare in locale. Non funzionano da un'altra macchina e non usare `.local`: passa da mDNS e sul tuo computer non punta a `127.0.0.1`.

## Se qualcosa non torna

Guarda la pagina dei dettagli del dominio (le tre righe con i passaggi) e la tabella nella [risoluzione dei problemi](./troubleshooting#domini).
