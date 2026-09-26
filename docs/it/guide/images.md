# Immagini al volo

OtterRoute può ridimensionare e convertire le immagini **al momento della richiesta**, senza che tu debba preparare tante versioni dello stesso file. Metti nel bucket l'originale ad alta risoluzione e chiedi la dimensione che serve nell'indirizzo:

```text
https://cdn.example.com/foto/barca.jpg?w=800&h=600&fit=cover&fmt=webp&q=80
```

L'originale non si modifica mai. Ogni variante si crea al primo accesso e da lì viene servita dalla cache su disco come qualunque altro file.

## Attivarle

Da **Instradamenti**, espandi la riga e nella sezione **Accesso e immagini** spunta *Immagini al volo*. La colonna **Accesso** mostra il badge *Immagini*. È una scelta per instradamento: senza, i parametri vengono ignorati e il file si serve com'è.

## I parametri

| Parametro | Valori | Cosa fa |
|---|---|---|
| `w` | 1–4096 | Larghezza massima in pixel. |
| `h` | 1–4096 | Altezza massima in pixel. |
| `fit` | `inside` (default), `cover` | `inside`: l'immagine sta dentro il riquadro `w`×`h` mantenendo le proporzioni. `cover`: riempie il riquadro e ritaglia al centro (serve `w` e `h`; con un solo lato coincide con `inside`). |
| `fmt` | `webp`, `jpeg`, `png`, `auto` | Formato di uscita. Senza `fmt` resta quello dell'originale (le GIF diventano PNG). `auto` sceglie WebP se il browser lo dichiara in `Accept`, altrimenti il formato dell'originale, e aggiunge `Vary: Accept`. |
| `q` | 30–95 (default 80) | Qualità per JPEG e WebP. Ignorata per PNG. |

Regole da conoscere:

- **Non si ingrandisce mai**: chiedere `w=3000` per un'immagine larga 1200 restituisce 1200. Con `cover`, se il riquadro sporge dall'originale si riduce mantenendo le sue proporzioni.
- L'**orientamento EXIF** delle foto si applica: una foto scattata in verticale resta in verticale.
- Il JPEG non ha trasparenza: le parti trasparenti diventano bianche.
- Un valore fuori intervallo o sconosciuto (`w=0`, `w=5000`, `fmt=avif`) dà `400`.
- I parametri estranei (`utm_source=…`) si ignorano come sempre.
- Vale solo per file `.jpg`, `.jpeg`, `.png`, `.gif` e `.webp`. Gli altri (SVG, PDF…) si servono senza trasformazione.

## Cache e costo

- Ogni combinazione di parametri è una **variante separata** in cache, con la stessa politica degli altri file (`X-Cache: HIT`/`MISS`, scadenza dopo 1 ora e rigenerazione). I parametri si normalizzano (`w=100&h=50` e `h=50&w=100` sono la stessa variante) e i limiti sopra impediscono un numero illimitato di varianti.
- L'**originale** si scarica dallo storage una sola volta e resta in cache: chiedere altre dimensioni dello stesso file non lo riscarica.
- La trasformazione è lavoro di CPU: il nodo ne esegue **poche alla volta** (metà dei core, tra 1 e 4) e le altre richieste aspettano. Con una variante già in cache il costo è quello di un file normale.
- Le richieste `Range` e `HEAD` funzionano sulle varianti.
- Con i [link firmati](./signed-links) il controllo della firma avviene prima; **i parametri dell'immagine non sono coperti dalla firma**: chi ha un link può chiedere qualunque dimensione consentita di *quel* file.

## Svuotare le varianti

**Svuota tutto** dell'instradamento ([Cache](./cache)) rende irraggiungibili anche tutte le varianti. **Svuota file** rimuove invece solo la copia dell'originale: le sue varianti restano fino alla scadenza (1 ora) o al prossimo *Svuota tutto*. Cambiare l'opzione *Immagini al volo* svuota da sola la cache dell'instradamento.

## Limiti di sicurezza

| Limite | Valore | Se lo superi |
|---|---|---|
| Dimensione dell'originale | 25 MiB | `413` |
| Pixel dell'originale | 64 milioni | `422` |
| Memoria per decodificare | 384 MiB | `422` |
| Tempo massimo per una trasformazione | 30 secondi | `503` |

Un file che non è davvero un'immagine (o è danneggiato) dà `422` e non viene messo in cache. Un originale inesistente dà `404` come sempre.

## Da provare

```sh
curl -sI 'https://cdn.example.com/foto/barca.jpg?w=400&fmt=webp' | grep -iE 'content-type|x-cache|content-length'
```

La prima richiesta è `MISS` e un po' più lenta (decodifica e ricodifica); la seconda è `HIT`.

## Limiti

- Niente AVIF (l'encoder è troppo lento per farlo al volo) né SVG.
- Le GIF animate diventano un'immagine statica (il primo fotogramma).
- Non ci sono filtri (sfocatura, filigrana) né ritagli su coordinate: solo ridimensionamento, `cover` centrato e conversione.
- Il nodo non guarda le altre cache: una CDN davanti deve includere la query (`?w=…`) e, con `fmt=auto`, l'header `Accept` nella sua chiave.
