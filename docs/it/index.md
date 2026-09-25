---
layout: home
title: OtterRoute
hero:
  name: OtterRoute
  text: I tuoi bucket S3, su tutti i tuoi domini
  tagline: Un gateway self-hosted che pubblica i file di più bucket privati attraverso più domini, con cache su disco. Colleghi uno storage, associ un dominio e il file è online.
  image:
    src: /brand/welcome.png
    alt: La lontra di OtterRoute che saluta
  actions:
    - theme: brand
      text: Come funziona
      link: /guide/how-it-works
    - theme: alt
      text: Installazione
      link: /guide/install
    - theme: alt
      text: Provider DNS e S3
      link: /providers/
features:
  - title: Più domini, più bucket
    details: Instradamento per host e prefisso di percorso. Ogni bucket ha le sue credenziali e la sua cache, isolata dalle altre.
  - title: Cache su disco
    details: Una sola richiesta allo storage per oggetto anche con molti client, streaming senza caricare i file in RAM, copie scadute servite se lo storage è giù.
  - title: Domini verificati davvero
    details: Il pannello controlla che il DNS risolva e che una richiesta al dominio arrivi a questo nodo. Se un dominio smette di funzionare, te lo segnala.
  - title: Utenti, permessi e 2FA
    details: Ruoli e scope sulle azioni, verifica in due passaggi con app di autenticazione, registro delle attività.
  - title: Statistiche e Prometheus
    details: Richieste, cache hit, banda, errori e latenza nel pannello, e le stesse metriche in formato Prometheus.
  - title: Sola lettura, per scelta
    details: Solo GET e HEAD, le credenziali restano sul nodo e la query string non arriva mai allo storage.
---
