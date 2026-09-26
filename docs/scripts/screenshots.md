# Come si rigenerano gli screenshot

Le immagini in `docs/public/screens/` sono catture del pannello su un'istanza demo con **dati finti** (chiavi `AK`/`SK`, un finto S3 locale, un utente di prova creato apposta): nessuna credenziale reale.

```sh
# 1. istanza demo
python3 scripts/fake-s3.py --root /tmp/dv/s3 --port 29392 --access-key AK --secret-key SK &
OTR_LISTEN=127.0.0.1:28380 OTR_ADMIN_LISTEN=127.0.0.1:29390 OTR_STATE_DIR=/tmp/dv/st \
  OTR_CACHE_DIR=/tmp/dv/c OTR_CONFIG=/tmp/dv/config.yaml OTR_UI_DIR=web/dist ./target/debug/otterroute &
# 2. si crea l'amministratore, due domini (img.localhost, media.localhost), un dominio
#    inesistente (cdn.example.invalid), due bucket, due instradamenti e qualche richiesta
#    (vedi scripts/panel-e2e.sh per le chiamate all'API)
# 3. si catturano le pagine a 1568×761 e si copiano in docs/public/screens/
```

Quando cambia l'interfaccia le catture vanno rifatte e la data nelle didascalie (`<figcaption>`) aggiornata.
