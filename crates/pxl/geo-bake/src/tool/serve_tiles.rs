// pxl-geo-bake/tool/serve_tiles.rs

//! The tile routes, for local development only.
//!
//! A deployment never answers these: terrain, POI and reverse-geocode pages
//! are baked offline and served out of the bucket, and the assets are there by
//! content hash. This bakes a page on demand instead, out of the same pipeline
//! the offline tool walks, so `deno task dev` draws what a deployment draws
//! with no bucket in the loop.
//!
//! It is a separate process from the live server on purpose. `pxl-geo` answers
//! forward search against three tables and is what production runs; this reads
//! the whole geo DB and is what a bake needs, and neither depends on the other
//! (issue/20260918-the-server-in-rust-and-the-line-already-drawn.md). Ordinary
//! work does not need it running at all — `dev:remote` serves published tiles
//! — so it is what one starts while working on the bake itself.
//!
//! Usage (from the repo root):
//!   cargo run --profile server -p pxl-geo-bake --bin serve_tiles -- [flags]
//!
//! Flags:
//!   --port <n>        listen port (default 3001)
//!   --host <addr>     bind address (default 127.0.0.1)
//!   --asset <dir>     ingested assets (default ./out/asset)
//!   --workers <n>     database connections, one per in-flight page (default 4)

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use postgres::{Client, NoTls};
use pxl_spec::bake::format::encode_poi_page;
use pxl_spec::space::indexer::L3_SHAMT;
use pxl_spec::space::pvf3::core::IVEC_WORD_MASK;
use pxl_spec::space::pvf3::ivec::IVec;

use pxl_geo_bake::geocode::provider::overture_provider::GeocodeProvider;
use pxl_geo_bake::poi::provider::overture_provider::PoiProvider;
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::{fetch_tile, SourceStats};

const MAX_L3_IDX: IVec = IVEC_WORD_MASK >> L3_SHAMT;

/// One worker: a connection and everything prepared against it. A page holds
/// one for the length of its bake, which is what makes the worker count and
/// the page-level concurrency the same number.
struct Worker {
    client: Client,
    terrain: TerrainRepository,
    poi: PoiProvider,
    geocode: GeocodeProvider,
    stats: SourceStats,
}

impl Worker {
    fn connect(url: &str) -> Result<Worker, postgres::Error> {
        let mut client = Client::connect(url, NoTls)?;
        Ok(Worker {
            terrain: TerrainRepository::prepare(&mut client)?,
            poi: PoiProvider::prepare(&mut client)?,
            geocode: GeocodeProvider::prepare(&mut client)?,
            client,
            stats: SourceStats::default(),
        })
    }
}

/// What a page answered with, kept so the next request for it does not bake
/// again. A page is a pure function of source data that does not change under
/// a running server, so there is nothing to invalidate on except a re-ingest —
/// which is what the internal route below is for.
#[derive(Clone)]
enum Cached {
    Bytes(Bytes),
    Json(Arc<str>),
    Absent,
}

struct App {
    workers: Mutex<Vec<Worker>>,
    terrain: Mutex<HashMap<IVec, Cached>>,
    poi: Mutex<HashMap<IVec, Cached>>,
    geocode: Mutex<HashMap<IVec, Cached>>,
    asset_dir: PathBuf,
    admin_token: Option<String>,
}

impl App {
    /// Takes a worker, runs `f` on it, and puts it back. Blocking by design:
    /// the whole bake is `Rc`-backed and synchronous, so it runs on a blocking
    /// thread and the async side only waits for it.
    fn with_worker<T>(&self, f: impl FnOnce(&mut Worker) -> T) -> Option<T> {
        let mut worker = loop {
            if let Some(w) = self.workers.lock().expect("workers").pop() {
                break w;
            }
            std::thread::yield_now();
        };
        let out = f(&mut worker);
        self.workers.lock().expect("workers").push(worker);
        Some(out)
    }
}

/// A page index is an unsigned L3 index. Rejecting the out-of-range ones here
/// matters more than it looks: the page maths happily shifts and unprojects a
/// nonsense value into a well-formed bbox with no error anywhere along the
/// way, and that would reach the database as a garbage query.
fn page_idx(raw: &str) -> Option<IVec> {
    if raw.is_empty() || raw.len() > 20 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse::<IVec>().ok().filter(|idx| *idx <= MAX_L3_IDX)
}

fn octet_stream(bytes: Bytes) -> Response {
    ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response()
}

async fn terrain(State(app): State<Arc<App>>, Path(raw): Path<String>) -> Response {
    let Some(idx) = page_idx(&raw) else {
        return (StatusCode::BAD_REQUEST, "bad page_idx").into_response();
    };

    if let Some(Cached::Bytes(b)) = app.terrain.lock().expect("cache").get(&idx).cloned() {
        return octet_stream(b);
    }
    let baked = tokio::task::spawn_blocking(move || {
        app.with_worker(|w| fetch_tile(&w.terrain, &mut w.client, idx, &mut w.stats).map(|t| Bytes::copy_from_slice(t.as_slice())))
            .expect("a worker")
            .inspect(|bytes| {
                app.terrain.lock().expect("cache").insert(idx, Cached::Bytes(bytes.clone()));
            })
            .map_err(|e| e.to_string())
    })
    .await
    .expect("bake task");

    match baked {
        Ok(bytes) => octet_stream(bytes),
        Err(err) => {
            eprintln!("[tile/terrain/{raw}] {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn poi(State(app): State<Arc<App>>, Path(raw): Path<String>) -> Response {
    let Some(idx) = page_idx(&raw) else {
        return (StatusCode::BAD_REQUEST, "bad page_idx").into_response();
    };

    if let Some(Cached::Bytes(b)) = app.poi.lock().expect("cache").get(&idx).cloned() {
        return octet_stream(b);
    }
    let baked = tokio::task::spawn_blocking(move || {
        app.with_worker(|w| {
            let pois = w.poi.fetch_pois(&mut w.client, idx).map_err(|e| e.to_string())?;
            let page = encode_poi_page(&pois).map_err(|e| format!("{e:?}"))?;
            Ok::<Bytes, String>(Bytes::copy_from_slice(page.as_slice()))
        })
        .expect("a worker")
        .inspect(|bytes| {
            app.poi.lock().expect("cache").insert(idx, Cached::Bytes(bytes.clone()));
        })
    })
    .await
    .expect("poi task");

    match baked {
        Ok(bytes) => octet_stream(bytes),
        Err(err) => {
            eprintln!("[tile/poi/{raw}] {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn geocode(State(app): State<Arc<App>>, Path(raw): Path<String>) -> Response {
    let Some(idx) = page_idx(&raw) else {
        return (StatusCode::BAD_REQUEST, "bad page_idx").into_response();
    };

    let json = |body: Arc<str>| ([(header::CONTENT_TYPE, "application/json")], body.to_string()).into_response();
    match app.geocode.lock().expect("cache").get(&idx).cloned() {
        Some(Cached::Json(body)) => return json(body),
        // A page outside Overture's Divisions coverage has no label, and the
        // client reads the 404 as "no label" rather than as an error.
        Some(Cached::Absent) => return (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "application/json")], "null").into_response(),
        _ => {}
    }

    let resolved = tokio::task::spawn_blocking(move || {
        app.with_worker(|w| w.geocode.reverse(&mut w.client, idx).map_err(|e| e.to_string())).expect("a worker").map(|result| {
            let cached = match &result {
                Some(r) => Cached::Json(Arc::from(serde_json::to_string(r).expect("geocode json").as_str())),
                None => Cached::Absent,
            };
            app.geocode.lock().expect("cache").insert(idx, cached.clone());
            cached
        })
    })
    .await
    .expect("geocode task");

    match resolved {
        Ok(Cached::Json(body)) => json(body),
        Ok(_) => (StatusCode::NOT_FOUND, [(header::CONTENT_TYPE, "application/json")], "null").into_response(),
        Err(err) => {
            eprintln!("[tile/geocode/{raw}] {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// A key is the sha256 of the object's bytes; the ingest writes each under its
/// type, so the content type is known without an extension.
const ASSET_TYPES: [(&str, &str); 3] = [("glb", "model/gltf-binary"), ("png", "image/png"), ("webp", "image/webp")];

async fn asset(State(app): State<Arc<App>>, Path(hash): Path<String>) -> Response {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    for (dir, content_type) in ASSET_TYPES {
        let path = app.asset_dir.join(dir).join(&hash);
        if let Ok(bytes) = tokio::fs::read(&path).await {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
            return (headers, bytes).into_response();
        }
    }
    (StatusCode::NOT_FOUND, "not found — run the asset ingest").into_response()
}

/// Drops what a re-ingest made stale. A page is a pure function of source data
/// that does not change under a running server, so nothing else invalidates.
async fn invalidate(State(app): State<Arc<App>>, headers: HeaderMap, Path(kind): Path<String>, body: String) -> Response {
    if let Some(token) = &app.admin_token {
        if headers.get("x-admin-token").and_then(|v| v.to_str().ok()) != Some(token.as_str()) {
            return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    }
    let cache = match kind.as_str() {
        "terrain" => &app.terrain,
        "poi" => &app.poi,
        "geocode" => &app.geocode,
        _ => return (StatusCode::NOT_FOUND, "unknown cache").into_response(),
    };

    let one = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("page_idx").and_then(|p| p.as_str().map(str::to_string)));
    match one {
        Some(raw) => match page_idx(&raw) {
            Some(idx) => {
                cache.lock().expect("cache").remove(&idx);
                (StatusCode::OK, format!("invalidated {raw}")).into_response()
            }
            None => (StatusCode::BAD_REQUEST, "bad page_idx").into_response(),
        },
        None => {
            cache.lock().expect("cache").clear();
            (StatusCode::OK, "invalidated all").into_response()
        }
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut port: u16 = 3001;
    let mut host = "127.0.0.1".to_string();
    let mut asset_dir = PathBuf::from("./out/asset");
    let mut workers: usize = 4;

    let mut i = 0;
    while i < argv.len() {
        let next = |i: &mut usize| -> Result<String, String> {
            *i += 1;
            argv.get(*i).cloned().ok_or_else(|| format!("{} wants a value", argv[*i - 1]))
        };
        match argv[i].as_str() {
            "--port" => port = next(&mut i)?.parse().map_err(|_| "bad --port")?,
            "--host" => host = next(&mut i)?,
            "--asset" => asset_dir = PathBuf::from(next(&mut i)?),
            "--workers" => workers = next(&mut i)?.parse().map_err(|_| "bad --workers")?,
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let mut pool = Vec::with_capacity(workers.max(1));
    for _ in 0..workers.max(1) {
        pool.push(Worker::connect(&url).map_err(|e| e.to_string())?);
    }
    println!("[tile] geo: ok ({} workers)", pool.len());

    let app = Arc::new(App {
        workers: Mutex::new(pool),
        terrain: Mutex::new(HashMap::new()),
        poi: Mutex::new(HashMap::new()),
        geocode: Mutex::new(HashMap::new()),
        asset_dir,
        // Absent in a dev process is not a hole: nothing outside the machine
        // reaches it, and requiring a token to clear a cache would only make
        // the loop it exists for harder to run.
        admin_token: std::env::var("ADMIN_TOKEN").ok().filter(|t| !t.is_empty()),
    });

    let router = Router::new()
        .route("/api/tiles/terrain/{page_idx}", get(terrain))
        .route("/api/tiles/poi/{page_idx}", get(poi))
        .route("/api/tiles/geocode/{page_idx}", get(geocode))
        .route("/api/tiles/asset/{hash}", get(asset))
        .route("/api/internal/world/{kind}/cache/invalidate", post(invalidate))
        .with_state(app);

    tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?.block_on(async move {
        let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await.map_err(|e| e.to_string())?;
        println!("[tile] listening on {host}:{port}");
        axum::serve(listener, router).await.map_err(|e| e.to_string())
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[tile] {err}");
            ExitCode::FAILURE
        }
    }
}
