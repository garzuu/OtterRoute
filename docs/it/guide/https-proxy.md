# HTTPS e proxy

::: tip Ti serve un proxy solo in alcuni casi
OtterRoute sa servire [HTTPS da solo](./https), con certificati gratuiti che si rinnovano in automatico. Un proxy o una CDN davanti ha senso se ce l'hai già, se il dominio passa da Cloudflare (la sfida dei certificati può funzionare, ma solo se il proxy la inoltra senza reindirizzarla: vedi [HTTPS automatico](./https)), se il nodo non può ascoltare sulla 443 o se vuoi un unico punto di ingresso per più servizi. In quel caso il proxy termina TLS e inoltra il traffico in HTTP al nodo.
:::

## Schema

```text
Visitatore ──HTTPS──▶ Proxy / CDN ──HTTP──▶ OtterRoute :80 ──▶ Storage S3
```

Il proxy deve **mantenere l'header `Host`** originale: OtterRoute sceglie l'instradamento dal nome del dominio.

::: tip La verifica dei domini
La verifica del pannello contatta **direttamente** gli indirizzi a cui il dominio risolve, in HTTP sulla porta 80, e si aspetta la risposta del nodo. Se il dominio risolve a un proxy che non inoltra il percorso `/.well-known/otterroute/check` al nodo, o che reindirizza tutto a HTTPS, la verifica mostrerà *"risponde un altro server"*. Il dominio funziona comunque per i visitatori: lascia passare quel percorso in HTTP se vuoi il segno di spunta verde.
:::

## Caddy

Il più semplice: ottiene e rinnova da solo i certificati.

```text
cdn.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Con OtterRoute su una porta diversa (`OTR_LISTEN=127.0.0.1:8080`) Caddy può occupare 80 e 443.

## nginx

```nginx
server {
    listen 443 ssl http2;
    server_name cdn.example.com;
    # ssl_certificate ... ; ssl_certificate_key ... ;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_buffering on;
    }
}
```

## Traefik

```yaml
http:
  routers:
    otterroute:
      rule: "Host(`cdn.example.com`)"
      entryPoints: [websecure]
      tls: { certResolver: letsencrypt }
      service: otterroute
  services:
    otterroute:
      loadBalancer:
        servers:
          - url: "http://127.0.0.1:8080"
```

(Questo blocco è una configurazione di Traefik, non di OtterRoute.)

## Cloudflare davanti al nodo

Con il proxy arancione di Cloudflare i visitatori usano HTTPS e Cloudflare inoltra al nodo. Tieni presente:

- Cloudflare inoltra in HTTP alla porta 80 solo con la modalità SSL **Flessibile**; in **Full** contatta l'origine in HTTPS (che il nodo non serve).
- Il DNS del dominio risolve agli indirizzi di Cloudflare, non al nodo: la verifica del pannello non troverà il nodo e resterà su "risponde un altro server". Per la verifica metti il record in **solo DNS** (nuvola grigia), oppure ignora l'avviso.
- Cloudflare mette in cache a sua volta: vedi anche la scheda [Cloudflare](/providers/dns/cloudflare).

## Cosa non fa il nodo

Non genera certificati, non legge `X-Forwarded-*`, non reindirizza da HTTP a HTTPS: tutto questo spetta al proxy.
