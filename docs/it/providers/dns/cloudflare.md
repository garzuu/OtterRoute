# Cloudflare

Come far puntare un dominio gestito su Cloudflare al tuo nodo OtterRoute.

## Cosa creare

Nel pannello di OtterRoute il dominio ti mostra i record da creare. In sostanza:

| Tipo | Nome | Valore |
|---|---|---|
| `A` | `cdn` (o il sottodominio scelto) | l'IPv4 pubblico del nodo (o del tuo proxy) |
| `AAAA` | idem | l'IPv6, se lo usi |
| `CNAME` | `cdn` | un nome che già punta al nodo (solo per sottodomini, mai per l'apex) |

## Passi

1. Dashboard → seleziona il dominio → **DNS** → **Records** → **Add record**.
2. Scegli `A` (o `AAAA`), inserisci nome e IP.
3. Decidi lo stato del proxy: **DNS only** (nuvola grigia) o **Proxied** (arancione).

## Attenzioni

- **Proxied (arancione)**: il DNS risponde con gli indirizzi di Cloudflare, non con il tuo IP. I visitatori arrivano a Cloudflare, che inoltra al nodo. La verifica del pannello non troverà il nodo e resterà su "risponde un altro server": per la verifica usa **DNS only** oppure ignora l'avviso. Con il proxy vale anche [HTTPS e proxy](/guide/https-proxy).
- **Apex**: Cloudflare consente un CNAME sull'apex grazie al *CNAME flattening* (risolve la catena e risponde con gli IP).
- Un CNAME verso un nome senza record A/AAAA produce risposte vuote, che sembrano un problema di propagazione.
- Cloudflare mette in cache a sua volta: la copia in CDN si somma a quella di OtterRoute.

## Verificare

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Poi premi **Ricontrolla** nel pannello: la verifica esegue proprio queste due prove (DNS, poi HTTP sulla porta 80 con la prova del nodo). Vedi [Domini e DNS](/guide/domains-dns) e [Risoluzione dei problemi](/guide/troubleshooting).

::: info Fonti ufficiali — verificato il 2026-09-26
- [CNAME flattening](https://developers.cloudflare.com/dns/cname-flattening/)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
