# Bucket S3

Un bucket è uno storage compatibile S3 (AWS, Cloudflare R2, Backblaze B2, Wasabi, MinIO…) da cui OtterRoute legge i file. Si aggiunge da **Bucket → Nuovo bucket**.

## I campi

| Campo | Cosa inserire |
|---|---|
| **Nome** | Un'etichetta per riconoscerlo. |
| **Endpoint** | Solo indirizzo dello storage: schema + host (+ porta). **Senza** bucket, percorso, query o credenziali nell'URL. Es. `https://s3.eu-central-1.amazonaws.com`. |
| **Regione** | La regione con cui si firma la richiesta (`us-east-1` di default). Deve coincidere con quella del bucket; alcuni provider usano un valore fisso (es. `auto`). |
| **Indirizzamento** | *Path* o *Virtual host*, vedi sotto. |
| **Access key / Secret key** | Una chiave di **sola lettura** (vedi "Permessi minimi"). Le chiavi restano sul nodo, in un file leggibile solo dal proprietario, e non vengono mai mostrate di nuovo. |
| **Bucket** | Il nome del bucket. |
| **File di prova** | Il percorso di un file che esiste davvero nel bucket, es. `foto/barca.jpg`. Serve per verificare la lettura ora e nei controlli successivi. |

Il pannello mostra la casella *"Lo storage è in rete locale"* solo per indirizzi interni (vedi [Sicurezza](./security)).

## Path oppure virtual host

Sono due modi di scrivere lo stesso URL:

| Stile | URL della richiesta | Tipico di |
|---|---|---|
| **Path** | `https://ENDPOINT/BUCKET/chiave` | MinIO, Garage, Ceph, molti storage self-hosted |
| **Virtual host** | `https://BUCKET.ENDPOINT/chiave` | AWS S3, Wasabi, DigitalOcean Spaces, Backblaze B2, Hetzner… |

Se la verifica dà un errore di firma o di host non trovato con uno stile, prova l'altro. Ogni [scheda provider](/providers/#storage-s3) dice quale usare.

## Permessi minimi (sola lettura)

OtterRoute **non scrive mai** nello storage. Crea una chiave dedicata con il solo permesso di leggere gli oggetti che devi pubblicare (`s3:GetObject` sul bucket o sulla cartella). Non serve `s3:ListBucket`:

- **senza** ListBucket, un file mancante fa rispondere `403 AccessDenied` a molti storage; OtterRoute lo trasforma in `404` per il visitatore;
- **con** ListBucket il file mancante dà `404 NoSuchKey`. Funziona in entrambi i casi.

Una chiave con più permessi del necessario è un rischio in più se il nodo venisse compromesso.

## Come si trova il file

L'instradamento aggiunge la **cartella** all'inizio del percorso: con il bucket `catalogo` e la cartella `foto/`, la richiesta `/barca.jpg` legge `foto/barca.jpg`. Il percorso viene normalizzato: `.` e `..` sono rifiutati, `%2F` (una barra codificata) è rifiutato perché ambiguo, le barre multiple si comprimono. Un percorso che finisce con `/` è una "cartella" e risponde `404`.

## Cosa può andare storto

| Messaggio | Causa più probabile |
|---|---|
| `403 SignatureDoesNotMatch` | Secret key sbagliata, **regione** sbagliata, stile di indirizzamento errato o **orologio** della macchina sfasato di oltre 15 minuti. |
| `403 InvalidAccessKeyId` | Access key sbagliata o di un altro provider/regione. |
| "Storage non raggiungibile" | Endpoint sbagliato, rete o firewall, DNS dello storage non risolve. |
| "il file non risulta accessibile" | Bucket, cartella o nome del file di prova sbagliati, oppure la chiave non può leggerlo. |
| "endpoint interno … serve allow_private_endpoint" | L'endpoint è un indirizzo locale: spunta la casella dedicata. |

OtterRoute **non segue i redirect** dello storage (per esempio un `301 PermanentRedirect` di AWS quando la regione è sbagliata): imposta la regione esatta.

Tutti i messaggi sono spiegati in [Risoluzione dei problemi](./troubleshooting#bucket).
