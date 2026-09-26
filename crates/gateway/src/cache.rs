//! Cache su disco locale al nodo.
//!
//! Struttura: `<dir>/ab/cd/<hash>.meta` (JSON) + `<hash>.body`.
//! - l'indice (dimensione e ultimo accesso) sta in memoria ed è ricostruito all'avvio
//! - le scritture passano da un file temporaneo e un rename: niente oggetti a metà
//! - quando si supera il limite si eliminano gli oggetti usati meno di recente
//! - l'invalidazione non cancella nulla: cambia la chiave (vedi routing::cache_key)

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use crate::config::CachePolicy;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// 200 oppure 404 (cache negativa)
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// secondi Unix
    pub stored_at: u64,
    pub size: u64,
}

impl Meta {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct Entry {
    pub meta: Meta,
    /// aperto al momento della lettura: resta valido anche se l'oggetto
    /// viene eliminato dalla cache mentre lo stiamo servendo
    pub file: Option<File>,
}

impl Entry {
    pub fn age(&self) -> Duration {
        Duration::from_secs(now_secs().saturating_sub(self.meta.stored_at))
    }

    fn ttl(&self, p: &CachePolicy) -> Duration {
        if self.meta.status == 404 {
            p.ttl_not_found
        } else {
            p.ttl
        }
    }

    pub fn is_fresh(&self, p: &CachePolicy) -> bool {
        self.age() < self.ttl(p)
    }

    /// Scaduta ma ancora servibile se lo storage non risponde.
    pub fn is_usable_stale(&self, p: &CachePolicy) -> bool {
        self.meta.status == 200 && self.age() < self.ttl(p) + p.serve_stale_on_error
    }
}

struct IdxEntry {
    size: u64,
    last_access: u64,
}

#[derive(Default)]
struct Index {
    entries: HashMap<String, IdxEntry>,
    total: u64,
}

pub struct Cache {
    dir: PathBuf,
    max_bytes: u64,
    index: Mutex<Index>,
    inflight: Mutex<HashMap<String, watch::Receiver<()>>>,
}

pub enum FillRole {
    /// questa richiesta scarica l'oggetto e lo mette in cache
    Leader(FillGuard),
    /// un'altra richiesta lo sta già scaricando: aspetta o prosegue senza cache
    Follower(watch::Receiver<()>),
}

/// Finché esiste, le altre richieste per la stessa chiave aspettano.
/// Quando viene rilasciato (fine download o errore) le risveglia.
pub struct FillGuard {
    cache: Arc<Cache>,
    key: String,
    _tx: watch::Sender<()>,
}

impl Drop for FillGuard {
    fn drop(&mut self) {
        self.cache.inflight.lock().unwrap().remove(&self.key);
    }
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub entries: usize,
    pub bytes: u64,
    pub max_bytes: u64,
    pub inflight: usize,
}

impl Cache {
    /// Apre la cache e ricostruisce l'indice dal disco.
    pub fn open(dir: impl Into<PathBuf>, max_bytes: u64) -> io::Result<Arc<Self>> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let mut idx = Index::default();
        scan(&dir, &mut idx)?;
        tracing::info!(entries = idx.entries.len(), bytes = idx.total, dir = %dir.display(), "cache caricata");
        let c = Arc::new(Cache {
            dir,
            max_bytes,
            index: Mutex::new(idx),
            inflight: Mutex::new(HashMap::new()),
        });
        c.evict_if_needed();
        Ok(c)
    }

    pub fn stats(&self) -> Stats {
        let i = self.index.lock().unwrap();
        Stats {
            entries: i.entries.len(),
            bytes: i.total,
            max_bytes: self.max_bytes,
            inflight: self.inflight.lock().unwrap().len(),
        }
    }

    fn paths(&self, key: &str) -> (PathBuf, PathBuf) {
        let d = self.dir.join(&key[0..2]).join(&key[2..4]);
        (d.join(format!("{key}.meta")), d.join(format!("{key}.body")))
    }

    pub async fn lookup(&self, key: &str) -> Option<Entry> {
        {
            let mut i = self.index.lock().unwrap();
            let e = i.entries.get_mut(key)?;
            e.last_access = now_millis();
        }
        let (mp, bp) = self.paths(key);
        let meta: Meta = match tokio::fs::read(&mp)
            .await
            .map(|b| serde_json::from_slice(&b))
        {
            Ok(Ok(m)) => m,
            _ => {
                self.forget(key);
                return None;
            }
        };
        let file = if meta.status == 200 {
            match File::open(&bp).await {
                Ok(f) => Some(f),
                Err(_) => {
                    self.forget(key);
                    return None;
                }
            }
        } else {
            None
        };
        Some(Entry { meta, file })
    }

    pub fn begin_fill(self: &Arc<Self>, key: &str) -> FillRole {
        let mut m = self.inflight.lock().unwrap();
        if let Some(rx) = m.get(key) {
            return FillRole::Follower(rx.clone());
        }
        let (tx, rx) = watch::channel(());
        m.insert(key.to_owned(), rx);
        FillRole::Leader(FillGuard {
            cache: self.clone(),
            key: key.to_owned(),
            _tx: tx,
        })
    }

    pub async fn writer(&self, key: &str) -> io::Result<CacheWriter> {
        let (mp, bp) = self.paths(key);
        tokio::fs::create_dir_all(bp.parent().unwrap()).await?;
        let tmp = bp.with_extension(format!(
            "body.tmp-{}",
            std::process::id() as u64 ^ now_millis()
        ));
        let file = File::create(&tmp).await?;
        Ok(CacheWriter {
            key: key.to_owned(),
            tmp,
            body: bp,
            meta: mp,
            file,
            written: 0,
        })
    }

    /// Cache negativa per "oggetto inesistente".
    pub async fn store_not_found(&self, key: &str) -> io::Result<()> {
        let (mp, bp) = self.paths(key);
        tokio::fs::create_dir_all(mp.parent().unwrap()).await?;
        let _ = tokio::fs::remove_file(&bp).await;
        write_meta(
            &mp,
            &Meta {
                status: 404,
                headers: vec![],
                stored_at: now_secs(),
                size: 0,
            },
        )
        .await?;
        self.insert(key, 0);
        Ok(())
    }

    /// Lo storage ha confermato (304) che la copia è ancora valida.
    pub async fn refresh(&self, key: &str, meta: &Meta) -> io::Result<()> {
        let (mp, _) = self.paths(key);
        let mut m = meta.clone();
        m.stored_at = now_secs();
        write_meta(&mp, &m).await
    }

    fn insert(&self, key: &str, size: u64) {
        {
            let mut i = self.index.lock().unwrap();
            let old = i.entries.insert(
                key.to_owned(),
                IdxEntry {
                    size,
                    last_access: now_millis(),
                },
            );
            if let Some(o) = old {
                i.total -= o.size;
            }
            i.total += size;
        }
        self.evict_if_needed();
    }

    /// Elimina una copia (metadati e corpo) dal disco e dall'indice. Restituisce
    /// se esisteva.
    pub async fn remove(&self, key: &str) -> bool {
        let existed = {
            let mut i = self.index.lock().unwrap();
            match i.entries.remove(key) {
                Some(o) => {
                    i.total -= o.size;
                    true
                }
                None => false,
            }
        };
        let (mp, bp) = self.paths(key);
        let _ = tokio::fs::remove_file(&mp).await;
        let _ = tokio::fs::remove_file(&bp).await;
        existed
    }

    fn forget(&self, key: &str) {
        let mut i = self.index.lock().unwrap();
        if let Some(o) = i.entries.remove(key) {
            i.total -= o.size;
        }
    }

    /// Elimina gli oggetti meno usati finché si scende al 90% del limite.
    fn evict_if_needed(&self) {
        let victims: Vec<String> = {
            let mut i = self.index.lock().unwrap();
            if i.total <= self.max_bytes {
                return;
            }
            let target = self.max_bytes - self.max_bytes / 10;
            let mut by_age: Vec<(u64, String, u64)> = i
                .entries
                .iter()
                .map(|(k, e)| (e.last_access, k.clone(), e.size))
                .collect();
            by_age.sort_unstable();
            let mut out = Vec::new();
            for (_, k, size) in by_age {
                if i.total <= target {
                    break;
                }
                i.entries.remove(&k);
                i.total -= size;
                out.push(k);
            }
            out
        };
        tracing::debug!(count = victims.len(), "eviction");
        for k in victims {
            let (mp, bp) = self.paths(&k);
            let _ = std::fs::remove_file(mp);
            let _ = std::fs::remove_file(bp);
        }
    }
}

pub struct CacheWriter {
    key: String,
    tmp: PathBuf,
    body: PathBuf,
    meta: PathBuf,
    file: File,
    written: u64,
}

impl CacheWriter {
    pub async fn write(&mut self, chunk: &[u8]) -> io::Result<()> {
        self.file.write_all(chunk).await?;
        self.written += chunk.len() as u64;
        Ok(())
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    pub async fn commit(
        mut self,
        cache: &Cache,
        status: u16,
        headers: Vec<(String, String)>,
    ) -> io::Result<()> {
        self.file.flush().await?;
        self.file.sync_data().await?;
        drop(self.file);
        tokio::fs::rename(&self.tmp, &self.body).await?;
        let meta = Meta {
            status,
            headers,
            stored_at: now_secs(),
            size: self.written,
        };
        write_meta(&self.meta, &meta).await?;
        cache.insert(&self.key, self.written);
        Ok(())
    }

    pub async fn abort(self) {
        drop(self.file);
        let _ = tokio::fs::remove_file(&self.tmp).await;
    }
}

async fn write_meta(path: &Path, meta: &Meta) -> io::Result<()> {
    let tmp = path.with_extension("meta.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec(meta)?).await?;
    tokio::fs::rename(&tmp, path).await
}

fn scan(dir: &Path, idx: &mut Index) -> io::Result<()> {
    for l1 in std::fs::read_dir(dir)? {
        let l1 = l1?.path();
        if !l1.is_dir() {
            continue;
        }
        for l2 in std::fs::read_dir(&l1)? {
            let l2 = l2?.path();
            if !l2.is_dir() {
                continue;
            }
            for f in std::fs::read_dir(&l2)? {
                let p = f?.path();
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_owned();
                if name.contains(".tmp") {
                    let _ = std::fs::remove_file(&p);
                    continue;
                }
                let Some(key) = name.strip_suffix(".meta") else {
                    continue;
                };
                let meta: Option<Meta> = std::fs::read(&p)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok());
                let body = p.with_extension("body");
                let ok = match &meta {
                    Some(m) if m.status == 404 => true,
                    Some(m) => std::fs::metadata(&body)
                        .map(|md| md.len() == m.size)
                        .unwrap_or(false),
                    None => false,
                };
                if !ok {
                    let _ = std::fs::remove_file(&p);
                    let _ = std::fs::remove_file(&body);
                    continue;
                }
                let last_access = std::fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let size = meta.map(|m| m.size).unwrap_or(0);
                idx.total += size;
                idx.entries
                    .insert(key.to_owned(), IdxEntry { size, last_access });
            }
        }
    }
    Ok(())
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> String {
        format!("{:02x}{}", n, "a".repeat(62))
    }

    #[tokio::test]
    async fn write_lookup_evict_and_rescan() {
        let dir = tempfile::tempdir().unwrap();
        let c = Cache::open(dir.path(), 25).unwrap();
        for n in 0..3u8 {
            let mut w = c.writer(&key(n)).await.unwrap();
            w.write(&[n; 10]).await.unwrap();
            w.commit(&c, 200, vec![("etag".into(), format!("\"{n}\""))])
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // 30 byte > 25: sparisce il meno recente
        assert!(c.lookup(&key(0)).await.is_none());
        let e = c.lookup(&key(2)).await.unwrap();
        assert_eq!(e.meta.header("ETag"), Some("\"2\""));
        assert_eq!(c.stats().bytes, 20);

        c.store_not_found(&key(9)).await.unwrap();
        drop(c);
        let c2 = Cache::open(dir.path(), 25).unwrap();
        assert_eq!(c2.stats().entries, 3);
        assert_eq!(c2.lookup(&key(9)).await.unwrap().meta.status, 404);
    }

    #[tokio::test]
    async fn remove_deletes_files_and_index() {
        let dir = tempfile::tempdir().unwrap();
        let c = Cache::open(dir.path(), 1000).unwrap();
        for n in 0..2u8 {
            let mut w = c.writer(&key(n)).await.unwrap();
            w.write(&[n; 10]).await.unwrap();
            w.commit(&c, 200, vec![]).await.unwrap();
        }
        assert_eq!(c.stats().bytes, 20);
        assert!(c.remove(&key(0)).await);
        assert!(!c.remove(&key(0)).await, "già rimossa");
        assert!(c.lookup(&key(0)).await.is_none());
        assert_eq!((c.stats().entries, c.stats().bytes), (1, 10));
        let (mp, bp) = c.paths(&key(0));
        assert!(!mp.exists() && !bp.exists());
        assert!(c.lookup(&key(1)).await.is_some(), "le altre restano");
        // dopo un riavvio la copia rimossa non ricompare
        drop(c);
        assert_eq!(Cache::open(dir.path(), 1000).unwrap().stats().entries, 1);
    }

    #[tokio::test]
    async fn fill_coalescing() {
        let dir = tempfile::tempdir().unwrap();
        let c = Cache::open(dir.path(), 1000).unwrap();
        let FillRole::Leader(g) = c.begin_fill("k") else {
            panic!()
        };
        let FillRole::Follower(mut rx) = c.begin_fill("k") else {
            panic!()
        };
        drop(g);
        // il leader ha finito: il follower si sveglia
        assert!(rx.changed().await.is_err());
        assert!(matches!(c.begin_fill("k"), FillRole::Leader(_)));
    }
}
