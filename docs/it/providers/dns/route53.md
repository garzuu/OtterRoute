# AWS Route 53

Come far puntare un dominio gestito su AWS Route 53 al tuo nodo OtterRoute.

## Cosa creare

Nel pannello di OtterRoute il dominio ti mostra i record da creare. In sostanza:

| Tipo | Nome | Valore |
|---|---|---|
| `A` | `cdn` (o il sottodominio scelto) | l'IPv4 pubblico del nodo (o del tuo proxy) |
| `AAAA` | idem | l'IPv6, se lo usi |
| `CNAME` | `cdn` | un nome che già punta al nodo (solo per sottodomini, mai per l'apex) |

## Passi

1. Console Route 53 → **Hosted zones** → la tua zona → **Create record**.
2. Tipo `A` (o `AAAA`), nome, valore = IP del nodo, TTL a piacere.
3. Salva.

## Attenzioni

- **Apex**: non si può creare un CNAME sull'apex. Route 53 offre i record **Alias**, ma solo verso risorse AWS (CloudFront, ELB, bucket S3 di tipo sito web, altro record della stessa zona…), **non** verso un IP o un dominio qualunque. Se il nodo non è su AWS, sull'apex usa un record `A` con il suo IP.
- Un record Alias verso una risorsa AWS non ha TTL modificabile: usa quello della risorsa.
- Un CNAME risponde per qualunque tipo di query; un Alias solo se nome e tipo coincidono.

## Verificare

```sh
dig +short cdn.example.com
curl -i http://cdn.example.com/.well-known/otterroute/check
```

Poi premi **Ricontrolla** nel pannello: la verifica esegue proprio queste due prove (DNS, poi HTTP sulla porta 80 con la prova del nodo). Vedi [Domini e DNS](/guide/domains-dns) e [Risoluzione dei problemi](/guide/troubleshooting).

::: info Fonti ufficiali — verificato il 2026-09-26
- [Alias e CNAME in Route 53](https://docs.aws.amazon.com/Route53/latest/DeveloperGuide/resource-record-sets-choosing-alias-non-alias.html)

La scheda riassume la documentazione del provider alla data indicata; i dettagli possono cambiare. **Non è stata provata dal vivo con un account reale.**
:::
