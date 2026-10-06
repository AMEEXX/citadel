use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;
use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use flate2::write::GzEncoder;
use flate2::Compression;

pub struct Asset {
    pub content_type: &'static str,
    pub identity: Bytes,
    pub gzip: Option<Bytes>,
    pub br: Option<Bytes>,
    pub etag: HeaderValue,
    pub hash8: &'static str,
}

fn compress_gzip(data: &[u8]) -> Option<Bytes> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data).ok()?;
    let compressed = encoder.finish().ok()?;
    if compressed.len() * 10 <= data.len() * 9 {
        // At least 10% saving
        Some(Bytes::from(compressed))
    } else {
        None
    }
}

#[allow(dead_code)]
fn compress_brotli(data: &[u8]) -> Option<Bytes> {
    let mut writer = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
    writer.write_all(data).ok()?;
    drop(writer);
    // Since CompressorWriter takes ownership of inner, use flate2 or simple helper if needed
    None
}

fn make_asset(content_type: &'static str, raw: &'static [u8]) -> Asset {
    let hash = blake3::hash(raw);
    let hash_hex = hash.to_hex();
    let etag_str = format!("\"b3-{}\"", &hash_hex.as_str()[..16]);
    let etag = HeaderValue::from_str(&etag_str).unwrap();

    let hash8_str = Box::leak(hash_hex.as_str()[..8].to_string().into_boxed_str());

    let is_compressible = content_type.starts_with("text/")
        || content_type.starts_with("application/javascript")
        || content_type.starts_with("application/json")
        || content_type.starts_with("image/svg+xml");

    let gzip = if is_compressible {
        compress_gzip(raw)
    } else {
        None
    };

    Asset {
        content_type,
        identity: Bytes::from_static(raw),
        gzip,
        br: None,
        etag,
        hash8: hash8_str,
    }
}

pub fn registry() -> &'static HashMap<&'static str, Asset> {
    static REGISTRY: OnceLock<HashMap<&'static str, Asset>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();

        m.insert("ace.bundle.js", make_asset("application/javascript; charset=utf-8", include_bytes!("../static/ace.bundle.js")));
        m.insert("citadel-skin.css", make_asset("text/css; charset=utf-8", include_bytes!("../static/citadel-skin.css")));
        m.insert("citadel-restore.js", make_asset("application/javascript; charset=utf-8", include_bytes!("../static/citadel-restore.js")));
        m.insert("fonts/citadel-fonts.css", make_asset("text/css; charset=utf-8", include_bytes!("../static/fonts/citadel-fonts.css")));

        let geist = make_asset("font/woff2", include_bytes!("../static/fonts/Geist-Variable.woff2"));
        let geist_mono = make_asset("font/woff2", include_bytes!("../static/fonts/GeistMono-Variable.woff2"));
        let inst_serif = make_asset("font/woff2", include_bytes!("../static/fonts/InstrumentSerif-Regular.woff2"));

        m.insert("fonts/Geist-Variable.woff2", geist);
        m.insert("fonts/GeistMono-Variable.woff2", geist_mono);
        m.insert("fonts/InstrumentSerif-Regular.woff2", inst_serif);

        // Dedupe C-4: InstrumentSerif-Italic is byte-identical to Regular
        let inst_italic = make_asset("font/woff2", include_bytes!("../static/fonts/InstrumentSerif-Regular.woff2"));
        m.insert("fonts/InstrumentSerif-Italic.woff2", inst_italic);

        m.insert("favicon.ico", make_asset("image/x-icon", include_bytes!("../static/favicon.ico")));
        m.insert("favicon-16x16.png", make_asset("image/png", include_bytes!("../static/favicon-16x16.png")));
        m.insert("favicon-32x32.png", make_asset("image/png", include_bytes!("../static/favicon-32x32.png")));
        m.insert("apple-touch-icon.png", make_asset("image/png", include_bytes!("../static/apple-touch-icon.png")));
        m.insert("citadel-badge.png", make_asset("image/png", include_bytes!("../static/citadel-badge.png")));
        m.insert("citadel-logo.png", make_asset("image/png", include_bytes!("../static/citadel-logo.png")));

        m
    })
}

pub fn versioned_url(path: &str) -> String {
    let clean = path.trim_start_matches('/');
    let clean_static = clean.strip_prefix("static/").unwrap_or(clean);
    if let Some(asset) = registry().get(clean_static) {
        format!("/static/{}?v={}", clean_static, asset.hash8)
    } else {
        format!("/static/{}", clean_static)
    }
}

pub fn respond(
    asset: &Asset,
    headers: &HeaderMap,
    is_versioned: bool,
) -> Response {
    let cache_enabled = std::env::var("CITADEL_NET_ASSET_CACHE")
        .map(|v| v != "0")
        .unwrap_or(true);
    let compression_enabled = std::env::var("CITADEL_NET_COMPRESSION")
        .map(|v| v != "0")
        .unwrap_or(true);

    if cache_enabled {
        if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
            if if_none_match.as_bytes() == asset.etag.as_bytes() {
                let cache_ctrl = if is_versioned {
                    "public, max-age=31536000, immutable"
                } else {
                    "public, max-age=0, must-revalidate"
                };
                return (
                    StatusCode::NOT_MODIFIED,
                    [
                        (header::ETAG, asset.etag.clone()),
                        (header::CACHE_CONTROL, HeaderValue::from_static(cache_ctrl)),
                        (header::VARY, HeaderValue::from_static("Accept-Encoding")),
                    ],
                )
                    .into_response();
            }
        }
    }

    let accept_encoding = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let (body, encoding) = if compression_enabled {
        if let Some(ref gz_data) = asset.gzip {
            if accept_encoding.contains("gzip") {
                (gz_data.clone(), Some("gzip"))
            } else {
                (asset.identity.clone(), None)
            }
        } else {
            (asset.identity.clone(), None)
        }
    } else {
        (asset.identity.clone(), None)
    };

    let cache_ctrl = if !cache_enabled {
        "public, max-age=31536000"
    } else if is_versioned {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=0, must-revalidate"
    };

    let mut res = (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(asset.content_type)),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache_ctrl)),
            (header::ETAG, asset.etag.clone()),
            (header::VARY, HeaderValue::from_static("Accept-Encoding")),
        ],
        body,
    )
        .into_response();

    if let Some(enc) = encoding {
        res.headers_mut().insert(header::CONTENT_ENCODING, HeaderValue::from_static(enc));
    }

    res
}
