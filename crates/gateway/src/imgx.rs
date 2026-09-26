//! Immagini al volo: `?w=800&h=600&fit=cover&fmt=webp&q=80`.
//!
//! Qui sta la parte pura (parametri, formato, trasformazione con limiti); il
//! collegamento con la cache e lo storage è in `handler.rs`. Le varianti sono
//! copie in cache come le altre, con una chiave che dipende dai parametri
//! normalizzati: parametri diversi, copie diverse, e mai più di quelle ammesse.

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageFormat, Limits};

/// Lato massimo richiesto (px).
pub const MAX_SIDE: u32 = 4096;
/// Massima dimensione dell'originale che si accetta di trasformare.
pub const MAX_SOURCE_BYTES: u64 = 25 * 1024 * 1024;
/// Massimo numero di pixel dell'originale: oltre è una "bomba di decompressione".
pub const MAX_SOURCE_PIXELS: u64 = 64_000_000;
const MAX_ALLOC: u64 = 384 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// sta dentro il riquadro w×h mantenendo le proporzioni
    Inside,
    /// riempie w×h ritagliando al centro
    Cover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Out {
    Jpeg,
    Png,
    Webp,
}

impl Out {
    pub fn content_type(self) -> &'static str {
        match self {
            Out::Jpeg => "image/jpeg",
            Out::Png => "image/png",
            Out::Webp => "image/webp",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Out::Jpeg => "jpeg",
            Out::Png => "png",
            Out::Webp => "webp",
        }
    }
}

/// Formato richiesto, prima di sapere cosa accetta il client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Want {
    /// come l'originale
    Original,
    /// WebP se il client lo accetta, altrimenti come l'originale
    Auto,
    Fixed(Out),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub fit: Fit,
    pub fmt: Want,
    pub q: u8,
}

pub const DEFAULT_Q: u8 = 80;

fn num(v: &str, min: u32, max: u32, name: &str) -> Result<u32, String> {
    let n: u32 = v.parse().map_err(|_| format!("{name} non valido"))?;
    if !(min..=max).contains(&n) {
        return Err(format!("{name} deve essere tra {min} e {max}"));
    }
    Ok(n)
}

/// Legge i parametri dalla query. `Ok(None)` se non ce n'è nessuno di immagine
/// (la richiesta va servita come un file qualunque); `Err` se un valore non è valido.
pub fn parse(query: Option<&str>) -> Result<Option<Params>, String> {
    let Some(q) = query else { return Ok(None) };
    let (mut w, mut h, mut fit, mut fmt, mut quality) =
        (None, None, Fit::Inside, Want::Original, DEFAULT_Q);
    let mut any = false;
    for (k, v) in url::form_urlencoded::parse(q.as_bytes()) {
        match &*k {
            "w" => (w, any) = (Some(num(&v, 1, MAX_SIDE, "w")?), true),
            "h" => (h, any) = (Some(num(&v, 1, MAX_SIDE, "h")?), true),
            "q" => (quality, any) = (num(&v, 30, 95, "q")? as u8, true),
            "fit" => {
                fit = match &*v {
                    "inside" | "contain" => Fit::Inside,
                    "cover" => Fit::Cover,
                    _ => return Err("fit deve essere inside o cover".into()),
                };
                any = true;
            }
            "fmt" => {
                fmt = match &*v {
                    "auto" => Want::Auto,
                    "webp" => Want::Fixed(Out::Webp),
                    "jpeg" | "jpg" => Want::Fixed(Out::Jpeg),
                    "png" => Want::Fixed(Out::Png),
                    _ => return Err("fmt deve essere auto, webp, jpeg o png".into()),
                };
                any = true;
            }
            _ => {}
        }
    }
    Ok(any.then_some(Params {
        w,
        h,
        fit,
        fmt,
        q: quality,
    }))
}

/// Estensioni che si sanno trasformare.
pub fn is_image_key(key: &str) -> bool {
    matches!(
        ext(key).as_deref(),
        Some("jpg" | "jpeg" | "png" | "gif" | "webp")
    )
}

fn ext(key: &str) -> Option<String> {
    key.rsplit('/')
        .next()?
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
}

/// Formato di uscita dopo aver visto il nome del file e cosa accetta il client.
pub fn resolve(p: &Params, key: &str, accept: Option<&str>) -> Out {
    let original = match ext(key).as_deref() {
        Some("png" | "gif") => Out::Png,
        Some("webp") => Out::Webp,
        _ => Out::Jpeg,
    };
    match p.fmt {
        Want::Fixed(o) => o,
        Want::Original => original,
        Want::Auto => {
            if accept.is_some_and(|a| a.contains("image/webp")) {
                Out::Webp
            } else {
                original
            }
        }
    }
}

/// La parte di chiave di cache che identifica la variante (parametri normalizzati).
pub fn canonical(p: &Params, out: Out) -> String {
    format!(
        "img:w={};h={};fit={};fmt={};q={}",
        p.w.map_or(String::new(), |v| v.to_string()),
        p.h.map_or(String::new(), |v| v.to_string()),
        if p.fit == Fit::Cover {
            "cover"
        } else {
            "inside"
        },
        out.name(),
        p.q
    )
}

/// Dimensioni finali senza mai ingrandire l'originale.
fn target(p: &Params, iw: u32, ih: u32) -> (u32, u32) {
    let (w, h) = (p.w.unwrap_or(u32::MAX), p.h.unwrap_or(u32::MAX));
    if p.w.is_none() && p.h.is_none() {
        return (iw, ih);
    }
    match p.fit {
        Fit::Inside => {
            let s = (f64::from(w) / f64::from(iw))
                .min(f64::from(h) / f64::from(ih))
                .min(1.0);
            (
                ((f64::from(iw) * s).round() as u32).max(1),
                ((f64::from(ih) * s).round() as u32).max(1),
            )
        }
        Fit::Cover => match (p.w, p.h) {
            (Some(w), Some(h)) => {
                // il riquadro w×h si riduce, con le sue proporzioni, se sporge dall'originale
                let s = (f64::from(iw) / f64::from(w))
                    .min(f64::from(ih) / f64::from(h))
                    .min(1.0);
                (
                    ((f64::from(w) * s).round() as u32).max(1),
                    ((f64::from(h) * s).round() as u32).max(1),
                )
            }
            // con un solo lato, cover coincide con inside
            _ => target(
                &Params {
                    fit: Fit::Inside,
                    ..*p
                },
                iw,
                ih,
            ),
        },
    }
}

fn flatten_on_white(img: &DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let mut out = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, px) in rgba.enumerate_pixels() {
        let a = u32::from(px[3]);
        let mix = |c: u8| ((u32::from(c) * a + 255 * (255 - a)) / 255) as u8;
        out.put_pixel(x, y, image::Rgb([mix(px[0]), mix(px[1]), mix(px[2])]));
    }
    out
}

/// Decodifica, applica l'orientamento EXIF, ridimensiona e ricodifica. Bloccante:
/// il chiamante la esegue in un thread dedicato.
pub fn transform(src: &[u8], p: &Params, out: Out) -> Result<Vec<u8>, String> {
    if src.len() as u64 > MAX_SOURCE_BYTES {
        return Err("originale troppo grande".into());
    }
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    let mut reader = image::ImageReader::new(Cursor::new(src))
        .with_guessed_format()
        .map_err(|e| format!("formato non riconosciuto: {e}"))?;
    match reader.format() {
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::Gif | ImageFormat::WebP) => {}
        _ => return Err("formato non supportato".into()),
    }
    reader.limits(limits);
    let mut dec = reader
        .into_decoder()
        .map_err(|e| format!("decodifica: {e}"))?;
    let (iw, ih) = dec.dimensions();
    if u64::from(iw) * u64::from(ih) > MAX_SOURCE_PIXELS {
        return Err("immagine con troppi pixel".into());
    }
    let orient = dec
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(dec).map_err(|e| format!("decodifica: {e}"))?;
    img.apply_orientation(orient);
    let (iw, ih) = (img.width(), img.height());
    let (tw, th) = target(p, iw, ih);
    let img = if (tw, th) == (iw, ih) {
        img
    } else if p.fit == Fit::Cover && p.w.is_some() && p.h.is_some() {
        img.resize_to_fill(tw, th, FilterType::Lanczos3)
    } else {
        img.resize_exact(tw, th, FilterType::Lanczos3)
    };
    let mut buf = Vec::new();
    match out {
        Out::Jpeg => {
            let rgb = flatten_on_white(&img);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, p.q)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .map_err(|e| format!("codifica JPEG: {e}"))?;
        }
        Out::Png => {
            let rgba = img.to_rgba8();
            image::codecs::png::PngEncoder::new(&mut buf)
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(|e| format!("codifica PNG: {e}"))?;
        }
        Out::Webp => {
            let rgba = img.to_rgba8();
            let enc = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
            buf = enc.encode(f32::from(p.q)).to_vec();
        }
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(w: u32, h: u32, fmt: ImageFormat, alpha: bool) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([
                (x * 255 / w) as u8,
                (y * 255 / h) as u8,
                128,
                if alpha && x < w / 2 { 0 } else { 255 },
            ]);
        }
        let mut out = Cursor::new(Vec::new());
        let dynimg = DynamicImage::ImageRgba8(img);
        match fmt {
            ImageFormat::Jpeg => DynamicImage::ImageRgb8(dynimg.to_rgb8())
                .write_to(&mut out, fmt)
                .unwrap(),
            _ => dynimg.write_to(&mut out, fmt).unwrap(),
        }
        out.into_inner()
    }

    fn dims(b: &[u8]) -> (u32, u32) {
        image::load_from_memory(b).unwrap().dimensions_pair()
    }

    trait Pair {
        fn dimensions_pair(&self) -> (u32, u32);
    }
    impl Pair for DynamicImage {
        fn dimensions_pair(&self) -> (u32, u32) {
            (self.width(), self.height())
        }
    }

    fn p(q: &str) -> Params {
        parse(Some(q)).unwrap().unwrap()
    }

    #[test]
    fn parsing_and_limits() {
        assert_eq!(parse(None), Ok(None));
        assert_eq!(
            parse(Some("v=2&utm=x")),
            Ok(None),
            "parametri estranei: file normale"
        );
        let a = p("w=800&h=600&fit=cover&fmt=webp&q=70");
        assert_eq!(
            (a.w, a.h, a.fit, a.fmt, a.q),
            (Some(800), Some(600), Fit::Cover, Want::Fixed(Out::Webp), 70)
        );
        assert_eq!(p("w=100").q, DEFAULT_Q);
        assert_eq!(p("fmt=auto").fmt, Want::Auto);
        for bad in [
            "w=0",
            "w=4097",
            "w=abc",
            "h=-1",
            "q=29",
            "q=96",
            "fit=stretch",
            "fmt=avif",
            "w=1e3",
            "w=",
        ] {
            assert!(parse(Some(bad)).is_err(), "{bad} deve essere rifiutato");
        }
        assert!(parse(Some("w=4096&h=4096&q=30")).is_ok());
    }

    #[test]
    fn keys_and_format_resolution() {
        assert!(
            is_image_key("foto/a.JPG")
                && is_image_key("x/b.webp")
                && !is_image_key("a.svg")
                && !is_image_key("a.pdf")
                && !is_image_key("noext")
        );
        let auto = p("w=10&fmt=auto");
        assert_eq!(
            resolve(&auto, "a.jpg", Some("image/avif,image/webp,*/*")),
            Out::Webp
        );
        assert_eq!(resolve(&auto, "a.jpg", Some("image/png")), Out::Jpeg);
        assert_eq!(resolve(&auto, "a.png", None), Out::Png);
        assert_eq!(resolve(&p("w=10"), "a.gif", None), Out::Png);
        assert_eq!(resolve(&p("w=10&fmt=jpeg"), "a.png", None), Out::Jpeg);
        // stessa variante, stessa chiave; parametri diversi, chiavi diverse
        let k = |q: &str, key: &str| canonical(&p(q), resolve(&p(q), key, None));
        assert_eq!(k("w=100&h=50", "a.jpg"), k("h=50&w=100", "a.jpg"));
        assert_ne!(k("w=100", "a.jpg"), k("w=101", "a.jpg"));
        assert_ne!(k("w=100&q=80", "a.jpg"), k("w=100&q=81", "a.jpg"));
        assert_ne!(k("w=100", "a.jpg"), k("w=100&fmt=webp", "a.jpg"));
    }

    #[test]
    fn resize_never_enlarges_and_keeps_aspect() {
        let src = sample(400, 300, ImageFormat::Jpeg, false);
        assert_eq!(
            dims(&transform(&src, &p("w=200"), Out::Jpeg).unwrap()),
            (200, 150)
        );
        assert_eq!(
            dims(&transform(&src, &p("h=150"), Out::Jpeg).unwrap()),
            (200, 150)
        );
        assert_eq!(
            dims(&transform(&src, &p("w=200&h=200"), Out::Jpeg).unwrap()),
            (200, 150),
            "inside"
        );
        assert_eq!(
            dims(&transform(&src, &p("w=2000"), Out::Jpeg).unwrap()),
            (400, 300),
            "niente ingrandimenti"
        );
        assert_eq!(
            dims(&transform(&src, &p("w=200&h=200&fit=cover"), Out::Jpeg).unwrap()),
            (200, 200),
            "cover ritaglia"
        );
        assert_eq!(
            dims(&transform(&src, &p("w=800&h=800&fit=cover"), Out::Jpeg).unwrap()),
            (300, 300),
            "cover non ingrandisce"
        );
        assert_eq!(
            dims(&transform(&src, &p("fmt=png"), Out::Png).unwrap()),
            (400, 300),
            "solo conversione"
        );
    }

    #[test]
    fn formats_and_quality() {
        let src = sample(320, 240, ImageFormat::Png, true);
        let jpeg = transform(&src, &p("w=160&fmt=jpeg"), Out::Jpeg).unwrap();
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        let png = transform(&src, &p("w=160&fmt=png"), Out::Png).unwrap();
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
        let webp = transform(&src, &p("w=160&fmt=webp"), Out::Webp).unwrap();
        assert!(webp.len() > 12 && &webp[..4] == b"RIFF" && &webp[8..12] == b"WEBP");
        assert_eq!(dims(&webp), (160, 120));
        // la trasparenza diventa bianca nel JPEG
        let j = image::load_from_memory(&jpeg).unwrap().to_rgb8();
        let left = j.get_pixel(2, 60);
        assert!(left[0] > 240 && left[1] > 240 && left[2] > 240, "{left:?}");
        // qualità più bassa, file più piccolo
        let big = sample(800, 600, ImageFormat::Jpeg, false);
        let hi = transform(&big, &p("w=600&q=95"), Out::Jpeg).unwrap();
        let lo = transform(&big, &p("w=600&q=30"), Out::Jpeg).unwrap();
        assert!(lo.len() < hi.len());
        let (wh, wl) = (
            transform(&big, &p("w=600&q=95&fmt=webp"), Out::Webp).unwrap(),
            transform(&big, &p("w=600&q=30&fmt=webp"), Out::Webp).unwrap(),
        );
        assert!(wl.len() < wh.len());
    }

    #[test]
    fn hostile_input_is_refused() {
        assert!(transform(b"non e' un'immagine", &p("w=10"), Out::Jpeg).is_err());
        assert!(transform(b"", &p("w=10"), Out::Jpeg).is_err());
        let src = sample(50, 50, ImageFormat::Png, false);
        assert!(
            transform(&src[..src.len() / 2], &p("w=10"), Out::Jpeg).is_err(),
            "file troncato"
        );
        // GIF/PNG con dimensioni enormi dichiarate: si rifiuta prima di allocare
        let mut bomb = sample(8, 8, ImageFormat::Png, false);
        // IHDR: larghezza e altezza a 100000 (il CRC non torna: la decodifica fallisce comunque)
        bomb[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        assert!(transform(&bomb, &p("w=10"), Out::Jpeg).is_err());
        let too_big = vec![0u8; (MAX_SOURCE_BYTES + 1) as usize];
        assert!(transform(&too_big, &p("w=10"), Out::Jpeg)
            .unwrap_err()
            .contains("troppo grande"));
    }
}
