# Google Cloud DNS

Come far puntare un dominio gestito su Google Cloud DNS al tuo nodo OtterRoute.

## Cosa creare

Nel pannello di OtterRoute il dominio ti mostra i record da creare. In sostanza:

| Tipo | Nome | Valore |
|---|---|---|
| `A` | `cdn` (o il sottodominio scelto) | l'IPv4 pubblico del nodo (o del tuo proxy) |
| `AAAA` | idem | l'IPv6, se lo usi |
| `CNAME` | `cdn` | un nome che già punta al nodo (solo per sottodomini, mai per l'apex) |

## Passi

1. Console → **Network services → Cloud DNS** → la tua zona → **Add standard**.
2. Nome DNS (lascia vuoto per l'apex), tipo `A`/`AAAA`/`CNAME`, TTL (obbligatorio), dati.
3. Crea. Con `gcloud`: `gcloud dns record-sets create cdn.example.com. --type=A --ttl=300 --rrdatas=IP --zone=ZONA`.

## Attenzioni

- **Punto finale**: i nomi senza il punto finale sono considerati relativi alla zona. In un CNAME scrivi sempre il nome completo con il punto (`nodo.example.net.`), altrimenti il valore viene esteso con il dominio della zona.
- **Apex**: i CNAME non sono ammessi sull'apex; usa `A`/`AAAA`. Il simbolo `@` non crea il record dell'apex: lascia il nome vuoto.
- Il TTL va indicato esplicitamente a ogni record.

## Verificare

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Poi premi **Ricontrolla** nel pannello: la verifica esegue proprio queste due prove (DNS, poi HTTP sulla porta 80 con la prova del nodo). Vedi [Domini e DNS](/guide/domains-dns) e [Risoluzione dei problemi](/guide/troubleshooting).

::: info Fonti ufficiali — verificato il 2026-09-26
- [Aggiungere record in Cloud DNS](https://docs.cloud.google.com/dns/docs/records)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
