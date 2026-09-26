# Pannello in HTTPS

Di norma il pannello risponde **solo sulla porta locale** (`127.0.0.1:9090`) e lo raggiungi con un tunnel SSH. Se preferisci usarlo da browser senza tunnel, puoi servirlo anche in **HTTPS su un dominio del nodo**, con lo stesso certificato e la stessa porta 443 dei tuoi siti.

::: warning Il pannello diventa raggiungibile da Internet
Sei tu a decidere di esporlo. Prima di attivarlo: password lunghe, [verifica in due passaggi obbligatoria](./users-2fa) (Utenti → Sicurezza) e, se puoi, un dominio poco prevedibile. Il blocco dopo 5 errori in 5 minuti protegge dai tentativi ripetuti, ma non sostituisce la 2FA.
:::

## Cosa serve

- Un **dominio dedicato** al pannello (per esempio `admin.example.com`), censito in *Domini*. Non deve avere instradamenti: il pannello e i file non si mescolano.
- Un **certificato in uso** per quel dominio: [automatico](./https) oppure caricato a mano. Senza certificato l'attivazione è rifiutata, perché il pannello non si aprirebbe.
- Il nodo **in ascolto su HTTPS** (`OTR_HTTPS_LISTEN`, di default la 443) e la porta raggiungibile dall'esterno.

## Attivarlo

**Impostazioni → Pannello in HTTPS**: scegli il dominio (compaiono solo quelli adatti) e premi *Attiva*. Il nodo mostra l'indirizzo da aprire e, se la 2FA non è obbligatoria, un avviso.

Da terminale, con lo stesso accesso di prima:

```sh
curl -s -b /tmp/otr.jar -H 'Content-Type: application/json' -X PUT \
  http://127.0.0.1:9090/api/admin-host -d '{"host":"admin.example.com"}'
```

Per disattivarlo: `-d '{"host":null}'` oppure *Disattiva* in Impostazioni.

## Come si comporta

- Su `https://admin.example.com/` risponde il pannello; il resto dei tuoi domini non cambia.
- L'HTTP di quel dominio reindirizza a HTTPS (`308`): il pannello **non si serve mai in chiaro**. Restano in HTTP le sfide dei certificati e la verifica del dominio, così l'emissione e il rinnovo continuano a funzionare.
- Il cookie di sessione diventa **`Secure`** e `HttpOnly`, `SameSite=Strict`.
- La porta locale `127.0.0.1:9090` continua a funzionare, indipendentemente da questa impostazione: se il certificato scade o qualcosa si rompe, non resti fuori.
- Non puoi eliminare il dominio né aggiungergli instradamenti finché il pannello è attivo su di esso.

## Limitare gli indirizzi

Puoi ammettere **solo certi indirizzi**: in *Impostazioni → Pannello in HTTPS* compare *Indirizzi ammessi*, un IP o una rete CIDR per riga (`203.0.113.7`, `10.0.0.0/8`, `2001:db8::/32`). Con l'elenco vuoto il pannello è aperto a tutti (con login).

- Chi non è nell'elenco riceve **404**, come se il dominio non avesse il pannello.
- La porta locale `127.0.0.1:9090` resta sempre aperta: se sbagli l'elenco, correggilo da lì (o con il tunnel SSH).
- Da HTTPS non puoi salvare un elenco che non comprende il tuo indirizzo.
- Il nodo guarda l'indirizzo della connessione, non gli header `X-Forwarded-For`: dietro Cloudflare vedrebbe gli IP di Cloudflare. In quel caso limita gli accessi con le regole di Cloudflare (Access, WAF).

```sh
curl -s -b /tmp/otr.jar -H 'Content-Type: application/json' -X PUT \
  http://127.0.0.1:9090/api/admin-allow -d '{"list":["203.0.113.0/24"]}'
```

## Dietro Cloudflare

Con la nuvola arancione, imposta SSL/TLS su **Full** (o Full strict con un certificato di origine). In **Flexible** Cloudflare parla al nodo in HTTP e il pannello reindirizzerebbe all'infinito. Il nodo non usa gli header `X-Forwarded-*`.

## Limiti

- Non c'è (ancora) un elenco di indirizzi ammessi: chi raggiunge il dominio vede la pagina di login. Per restringere l'accesso usa il firewall del server o Cloudflare Access.
- Vale un solo dominio per il pannello.
- Il certificato scaduto rende il pannello irraggiungibile su quel dominio: usa il tunnel locale per ripristinarlo.
