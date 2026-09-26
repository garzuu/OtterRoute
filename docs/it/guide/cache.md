# Cache

Ogni file richiesto viene tenuto su disco. Le richieste successive partono dal disco senza toccare lo storage.

![Sezione «Cache» della riga di un instradamento: svuota e precarica](/screens/routes.jpg)
<p class="shot-caption">Sezione «Cache» della riga di un instradamento: svuota e precarica · 26/09/2026</p>

<!--@include: @/diagrams/cache-decision.it.svg-->

## Cosa dice `X-Cache`

| Valore | Significato |
|---|---|
| `HIT` | Servito da una copia fresca in cache. |
| `MISS` | Non era in cache: scaricato dallo storage e salvato mentre veniva inviato. |
| `REVALIDATED` | Copia scaduta, ma lo storage ha confermato (`304`) che è ancora valida. |
| `STALE` | Copia scaduta servita perché lo storage non risponde (vedi sotto). |
| `BYPASS` | Passato allo storage senza usare né riempire la cache (richieste `HEAD` o con `Range` senza copia in cache, file troppo grandi). |
| *(assente)* | Risposta senza esito di cache: `404` generati dal gateway, errori. |

## La politica di cache

Il pannello usa una politica `standard`:

| Parametro | Valore | Cosa fa |
|---|---|---|
| `ttl` | **1 ora** | Per quanto una copia è fresca. Poi si rivalida con lo storage. |
| `ttl_not_found` | **60 secondi** | Ricorda per poco che un file manca (cache negativa), così un errore di battitura ripetuto non martella lo storage. |
| `serve_stale_on_error` | **24 ore** | Se lo storage non risponde, una copia scaduta può essere servita fino a questo limite (`STALE`). |
| `query_keys` | *nessuno* | Nessun parametro di query conta: `?v=2` e `?v=3` sono la stessa cosa. |

Per cambiare questi valori si modifica la configurazione a mano (vedi [config.yaml](/reference/config)); il pannello non li espone ancora.

## Comportamento utile da sapere

- **Una sola richiesta per oggetto.** Con molti visitatori sullo stesso file non ancora in cache, uno solo va allo storage; gli altri aspettano fino a **5 secondi** e poi, se serve, vanno anche loro allo storage senza cache.
- **Streaming.** I file non vengono caricati in RAM. Se il visitatore si disconnette il download continua per completare la cache.
- **Limite di spazio (LRU).** Oltre `OTR_CACHE_MAX_BYTES` (default **10 GiB**) le copie meno usate vengono eliminate.
- **File grandi.** Oltre `OTR_CACHE_MAX_OBJECT_BYTES` (default **1 GiB**) il file passa allo storage senza cache.
- **Isolamento.** La cache è separata per instradamento: lo stesso file servito da due domini ha due copie.
- **Intestazioni HTTP.** Sono gestiti `ETag`, `If-None-Match`, `If-Modified-Since`, `Range`, `If-Range`; gli header `x-amz-*` dello storage non arrivano mai al visitatore.
- **Errori.** Un `403 AccessDenied` dello storage per un file mancante diventa `404`; credenziali sbagliate danno `502` e **non** vengono mai messe in cache.

## Svuotare e precaricare dal pannello

Da **Instradamenti**, espandi la riga di un instradamento: nella sezione **Cache** trovi tre azioni (serve lo scope `routes:write`).

| Azione | Cosa fa |
|---|---|
| **Svuota file** | Elimina la copia di un solo file (scrivi il nome, es. `barca.jpg`): la richiesta successiva lo rilegge dallo storage. Ti dice se il file era in cache. |
| **Svuota tutto** | Svuota la cache dell'instradamento con un click e una conferma. Non cancella nulla dal disco: aumenta il `cache_generation` dell'instradamento, quindi le vecchie copie diventano irraggiungibili e l'LRU le elimina. |
| **Precarica** | Elenca fino a 200 file, uno per riga: il nodo li richiede da solo, uno alla volta, così la prima visita è già `HIT`. Per ogni file vedi stato, `X-Cache`, byte e tempo. Il precaricamento si interrompe dopo 2 minuti; i file rimasti sono segnalati come non eseguiti. |

Se l'instradamento ha le [immagini al volo](./images), *Svuota file* rimuove solo l'originale: le varianti scadono da sole o si eliminano con *Svuota tutto*. Svuotare non elimina il file dallo storage né dalle eventuali cache davanti al nodo (CDN, browser): quelle hanno i loro tempi.

Nella configurazione a mano lo stesso effetto si ottiene aumentando `cache_generation` di una destinazione.

## Limiti noti

- `HEAD` e `Range` senza una copia in cache passano allo storage senza riempirla: il primo accesso a un video "a pezzi" non lo mette in cache.
- Non c'è `index.html` automatico per le cartelle.
- Il precaricamento scarica interamente ogni file: con file molto grandi (fino al limite `OTR_CACHE_MAX_OBJECT_BYTES`) può richiedere tempo.
