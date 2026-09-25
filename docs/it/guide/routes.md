# Instradamenti

Un instradamento collega un **dominio + prefisso di percorso** a un **bucket + cartella**. Si crea da **Instradamenti → Nuovo instradamento** scegliendo tra i domini verificati e i bucket censiti.

## Come si legge

| Campo | Significato |
|---|---|
| **Dominio** | Deve essere verificato. |
| **Prefisso del percorso** | `/` per tutto il dominio, oppure `/docs/`. Il pannello lo normalizza (inizio e fine con `/`). |
| **Bucket** | Il bucket da cui leggere. |
| **Cartella** | Opzionale: tutto ciò che sta fuori resta privato. |

L'anteprima nella finestra mostra la corrispondenza: `img.azienda.it/file.jpg → catalogo/foto/file.jpg`.

## Esempi

| Instradamento | Richiesta | File letto |
|---|---|---|
| `img.azienda.it` + `/` → `catalogo` / `foto/` | `img.azienda.it/barca.jpg` | `catalogo/foto/barca.jpg` |
| `media.azienda.it` + `/docs/` → `documenti` / `pubblici/` | `media.azienda.it/docs/listino.pdf` | `documenti/pubblici/listino.pdf` |
| `media.azienda.it` + `/foto/` → `catalogo` / `foto/` | `media.azienda.it/foto/barca.jpg` | `catalogo/foto/barca.jpg` |

Lo stesso dominio può avere più instradamenti con prefissi diversi, anche verso bucket diversi.

## Regole di scelta

- **Vince il prefisso più lungo**, confrontato per segmenti interi: `/docs/` non intercetta `/docsx/`.
- Una coppia **dominio + prefisso** è unica: un duplicato viene rifiutato con il nome della regola che lo occupa.
- Un **host sconosciuto** risponde `404`.
- Sono ammessi solo `GET` e `HEAD` (altri metodi: `405`).
- Il percorso è normalizzato; `.`/`..` e `%2F` sono rifiutati con `400`.
- La **query string non viene inoltrata** allo storage e non entra nella chiave di cache.
- Il prefisso dell'instradamento viene tolto prima di cercare il file (nell'esempio del listino, `/docs/` non fa parte del nome del file nel bucket).

Le modifiche si applicano subito, senza riavvii. Per eliminare un dominio o un bucket bisogna prima eliminare gli instradamenti che li usano.
