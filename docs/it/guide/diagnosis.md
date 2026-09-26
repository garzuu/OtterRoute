# Diagnosi

Quando un file non si apre, la pagina **Diagnosi** segue il percorso della richiesta e ti dice **dove si ferma e cosa fare**. Basta incollare l'indirizzo del file, per esempio `https://cdn.example.com/foto/barca.jpg` (serve lo scope `routes:read`). Dalla riga di un instradamento il pulsante **Diagnosi** apre la pagina già compilata.

![Diagnosi di un file: ogni passaggio con il suo esito](/screens/diagnosis.jpg)
<p class="shot-caption">Diagnosi di un file: ogni passaggio con il suo esito · 26/09/2026</p>

## I passaggi

| Passaggio | Cosa controlla | Se fallisce |
|---|---|---|
| **Indirizzo** | Che sia un URL valido. Con `https://` controlla che il nodo abbia un certificato per il dominio e sia in ascolto su HTTPS; altrimenti avvisa che HTTPS dipende da un proxy davanti. | Correggi l'indirizzo. |
| **Il dominio arriva a questo nodo** | La stessa verifica dei domini: il nome risolve e una richiesta HTTP al dominio raggiunge *questo* nodo. Avvisa se il dominio funziona ma non è censito. | Record DNS, firewall, proxy: vedi [Domini e DNS](./domains-dns). |
| **Instradamento** | Quale regola serve il percorso (vince il prefisso più lungo) e quale file cerca nel bucket. Se non c'è, elenca i prefissi del dominio. | Crea l'instradamento o usa un prefisso esistente. Un percorso che finisce con `/` è una cartella: non si elenca. |
| **Cache** | Se il file è già in cache, fresco o scaduto, o se c'è un «non trovato» ricordato. | Se il file ora esiste, attendi la cache negativa o [svuotala](./cache). |
| **Storage** | Legge davvero il file dal bucket con la chiave salvata: tempo, dimensione, tipo. Distingue file mancante, credenziali rifiutate, storage irraggiungibile. | Vedi [Bucket S3](./buckets-s3) e [Risoluzione dei problemi](./troubleshooting). |
| **Risposta del nodo** | Una richiesta reale al nodo: stato HTTP, `X-Cache` e tempo. | Guarda i log del nodo. |

Il primo passaggio che fallisce è la causa più probabile: i successivi che ne dipendono risultano **saltati**. Ogni problema ha un riquadro **Cosa fare**.

::: info La prova è una richiesta vera
L'ultimo passaggio chiede il file al nodo come farebbe un visitatore: se esiste finisce in cache. Non modifica nulla nello storage.
:::

## Il report

**Copia report** mette negli appunti un testo con l'indirizzo, il nodo, la data, l'esito di ogni passaggio e il rimedio, da incollare in una segnalazione o in una chat. Non contiene chiavi né altri segreti: solo gli esiti.

## Limiti

- La diagnosi parte **dal nodo**: vede il DNS e la rete come li vede lui, che può essere diverso da come li vede il tuo browser (cache DNS locale, VPN, `/etc/hosts`).
- Con una configurazione scritta a mano la verifica dello storage non è disponibile (il pannello non ha le chiavi).
- Non segue i parametri di query e non prova HTTPS: se il proxy davanti al nodo è la causa, lo dice solo come avvertenza.
