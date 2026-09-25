# Aruba

Come far puntare un dominio gestito su Aruba al tuo nodo OtterRoute.

## Cosa creare

Nel pannello di OtterRoute il dominio ti mostra i record da creare. In sostanza:

| Tipo | Nome | Valore |
|---|---|---|
| `A` | `cdn` (o il sottodominio scelto) | l'IPv4 pubblico del nodo (o del tuo proxy) |
| `AAAA` | idem | l'IPv6, se lo usi |
| `CNAME` | `cdn` | un nome che già punta al nodo (solo per sottodomini, mai per l'apex) |

## Passi

1. Accedi al pannello di gestione (admin.aruba.it) → il dominio → **Gestione DNS**.
2. **Aggiungi record**, scegli il tipo (`A`, `AAAA`, `CNAME`), inserisci host e destinazione, TTL.
3. Conferma e salva la configurazione. Puoi anche programmare l'attivazione a una data futura.

## Attenzioni

- **Propagazione**: Aruba indica circa 24 ore.
- Puoi avere TTL diversi per host diversi dello stesso dominio.
- **Apex**: dalle fonti consultate non risulta un tipo di record alias/flattening: sull'apex usa `A`/`AAAA`.
- Le guide di Aruba variano tra hosting, domini e Aruba Business: cerca quella del tuo servizio.
- Il pannello usa i name server di Aruba solo se il dominio li utilizza.

## Verificare

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Poi premi **Ricontrolla** nel pannello: la verifica esegue proprio queste due prove (DNS, poi HTTP sulla porta 80 con la prova del nodo). Vedi [Domini e DNS](/guide/domains-dns) e [Risoluzione dei problemi](/guide/troubleshooting).

::: info Fonti ufficiali — verificato il 2026-09-26
- [Gestire un record CNAME (guide.aruba.it)](https://guide.aruba.it/hosting-e-domini/gestione-dns/gestione-name-server-e-record/gestire-record-cname)
- [Guida al pannello DNS di Aruba](https://www.aruba.it/magazine/hosting/la-gestione-del-pannello-dns-di-aruba.aspx)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
