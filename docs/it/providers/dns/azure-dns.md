# Azure DNS

Come far puntare un dominio gestito su Azure DNS al tuo nodo OtterRoute.

## Cosa creare

Nel pannello di OtterRoute il dominio ti mostra i record da creare. In sostanza:

| Tipo | Nome | Valore |
|---|---|---|
| `A` | `cdn` (o il sottodominio scelto) | l'IPv4 pubblico del nodo (o del tuo proxy) |
| `AAAA` | idem | l'IPv6, se lo usi |
| `CNAME` | `cdn` | un nome che già punta al nodo (solo per sottodomini, mai per l'apex) |

## Passi

1. Portale → **DNS zones** → la tua zona → **Record sets → Add**.
2. Nome, tipo `A`/`AAAA`/`CNAME`, TTL, indirizzo IP.
3. Salva.

## Attenzioni

- **Apex**: il DNS non ammette CNAME sull'apex. Azure offre i record **Alias**, ma verso risorse Azure (IP pubblico, Traffic Manager, CDN, Front Door, altro record della zona). Verso un nodo esterno usa un record `A` con il suo IP.
- Se il nodo ha un IP pubblico Azure di tipo Standard, un record A/AAAA di tipo *alias* lo segue quando l'IP cambia.
- Per creare record alias serve il resource provider **Microsoft.Network** registrato.

## Verificare

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Poi premi **Ricontrolla** nel pannello: la verifica esegue proprio queste due prove (DNS, poi HTTP sulla porta 80 con la prova del nodo). Vedi [Domini e DNS](/guide/domains-dns) e [Risoluzione dei problemi](/guide/troubleshooting).

::: info Fonti ufficiali — verificato il 2026-09-26
- [Alias records in Azure DNS](https://learn.microsoft.com/en-us/azure/dns/dns-alias)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
