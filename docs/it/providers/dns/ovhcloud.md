# OVHcloud

Come far puntare un dominio gestito su OVHcloud al tuo nodo OtterRoute.

## Cosa creare

Nel pannello di OtterRoute il dominio ti mostra i record da creare. In sostanza:

| Tipo | Nome | Valore |
|---|---|---|
| `A` | `cdn` (o il sottodominio scelto) | l'IPv4 pubblico del nodo (o del tuo proxy) |
| `AAAA` | idem | l'IPv6, se lo usi |
| `CNAME` | `cdn` | un nome che già punta al nodo (solo per sottodomini, mai per l'apex) |

## Passi

1. Area cliente → **Web Cloud → Domini** → il dominio → scheda **Zona DNS** → **Aggiungi un record**.
2. Scegli `A`/`AAAA`/`CNAME`, sottodominio e destinazione, TTL (predefinito o personalizzato).
3. Conferma.

## Attenzioni

- **Punto finale nei CNAME**: se la destinazione è un nome, scrivilo con il punto (`nodo.example.net.`); senza, OVHcloud aggiunge il tuo dominio in coda.
- **Propagazione**: OVHcloud indica fino a 24 ore. Per accelerare le modifiche future puoi ridurre il TTL predefinito dal menu *Azioni sulla zona*.
- Se il dominio usa name server diversi da quelli OVHcloud, la zona qui non ha effetto.

## Verificare

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Poi premi **Ricontrolla** nel pannello: la verifica esegue proprio queste due prove (DNS, poi HTTP sulla porta 80 con la prova del nodo). Vedi [Domini e DNS](/guide/domains-dns) e [Risoluzione dei problemi](/guide/troubleshooting).

::: info Fonti ufficiali — verificato il 2026-09-26
- [Modificare una zona DNS OVHcloud](https://docs.ovhcloud.com/en/guides/web-cloud/domains/dns-zone-edit)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
