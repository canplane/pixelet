// pxl-geo-bake/bin/serve_tiles.rs

//! The tile routes, for local development only.
//!
//! A deployment never answers these: terrain, POI and reverse-geocode pages
//! are baked offline and served out of the bucket, and the assets are there by
//! content hash. This bakes a page on demand instead, out of the same pipeline
//! the offline tool walks, so `deno task dev` draws what a deployment draws
//! with no bucket in the loop.
//!
//! Terrain is demand-paged. The backing store is the bake itself — a leaf
//! from the database by `terrain/leaf.rs`, a level above by folding the
//! sixteen pages under it (`terrain/pyramid.rs`), the functions `bake_tiles`
//! runs — and the page cache is a tile set on disk, laid out as the bucket is.
//! A page not yet on disk is a fault: its dependencies fault first, in
//! parallel, a page wanted twice at once is made once, and what is made is
//! written before it is served. So a page here holds the packet `bake_tiles`
//! would write — packed at the live level rather than the stored one, which is
//! the transport's choice and not the page's — and a batch bake is every page
//! faulted. The cache is dropped whole,
//! at start and on invalidation, never a page at a time: one page from before
//! a change beside one from after is a pyramid no bake would produce.
//!
//! Or it serves a finished bake: `--baked` answers terrain out of
//! `out/terrain/<published version>` as the bucket would, every level already
//! there, for work on the view rather than on the bake — zooming out to where
//! a page folds a million leaves, which no request could wait for. Demand
//! paging refuses the levels above `--top-level` for the same reason, with a
//! status the client retries rather than the 404 it would paint as sea.
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
//!   --cache <dir>     the terrain page cache (default ./out/dev/terrain/<published version>)
//!   --top-level <n>   the coarsest level demand paging makes (default 3)
//!   --baked           serve ./out/terrain/<published version> instead of baking

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use postgres::{Client, NoTls};
use pxl_spec::bake::constant::PAGE_TOP_LEVEL;
use pxl_spec::bake::format::{encode_poi_page, encode_terrain, Terrain};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::route::{ASSET_ROOT, GEO_INTERNAL, TILE_ROOT};
use pxl_spec::space::indexer::L3_SHAMT;
use pxl_spec::space::pvf3::core::IVEC_WORD_MASK;
use pxl_spec::space::pvf3::ivec::IVec;

use pxl_geo_bake::geocode::provider::overture::GeocodeProvider;
use pxl_geo_bake::poi::provider::overture::PoiProvider;
use pxl_geo_bake::terrain::leaf::{bake_leaf, bake_ocean, has_land, ocean_samples, Coverage};
use pxl_geo_bake::terrain::pyramid::{fold_stored, neighbourhood};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::SourceStats;
use pxl_geo_bake::terrain::tile_set::{read_page, TileSet};
use pxl_geo_bake::util::{pack, Level, CONTENT_ENCODING};
use tokio::sync::OnceCell;

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

/// A page of the terrain pyramid, by level and key.
type PageKey = (u32, IVec);

/// Whether a page exists, once it has been settled; the error a failed fault
/// ended in otherwise, which the next request for the page sees too until the
/// cache is dropped.
type Settled = Result<bool, String>;

/// The terrain page cache and the faults in flight against it. Each
/// invalidation opens a new generation in a directory of its own, so a fault
/// still running against the old one writes where nothing reads.
struct Pager {
    generation: u32,
    tiles: TileSet,
    coverage: Coverage,
    ocean: Terrain,
    ocean_packed: Bytes,
    faults: Mutex<HashMap<PageKey, Arc<OnceCell<Settled>>>>,
    /// Leaves baked from the database since the cache opened, for the log's
    /// word on a fault that runs long: a coarse page asks for every leaf
    /// under it and its rim.
    leaves: std::sync::atomic::AtomicU64,
}

impl Pager {
    /// An empty cache under `base`, with the ocean page in it.
    fn open(base: &std::path::Path, generation: u32, coverage: Coverage) -> Result<Pager, String> {
        let tiles = TileSet::new(base.join(generation.to_string()));
        std::fs::create_dir_all(&tiles.root).map_err(|e| e.to_string())?;
        let raw = bake_ocean(&ocean_samples(&coverage))?;
        let packed = pack(&raw, Level::Live).map_err(|e| e.to_string())?;
        std::fs::write(tiles.ocean_path(), &packed).map_err(|e| e.to_string())?;
        Ok(Pager {
            generation,
            ocean: read_page(&tiles.ocean_path())?,
            ocean_packed: Bytes::from(packed),
            tiles,
            coverage,
            faults: Mutex::new(HashMap::new()),
            leaves: std::sync::atomic::AtomicU64::new(0),
        })
    }
}

/// A finished tile set, served as it lies: its objects as the bake packed them,
/// under the encoding its manifest records.
struct Baked {
    tiles: TileSet,
    encoding: HeaderValue,
    ocean: Bytes,
}

impl Baked {
    fn open(root: PathBuf) -> Result<Baked, String> {
        let manifest_path = root.join("../manifest.json");
        let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path).map_err(|e| format!("{}: {e}", manifest_path.display()))?).map_err(|e| e.to_string())?;
        let version = root.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if manifest["version"].as_str() != Some(version) {
            return Err(format!("{}: records {}, not {version}", manifest_path.display(), manifest["version"]));
        }
        let encoding = manifest["content_encoding"].as_str().ok_or("the manifest names no content_encoding")?;
        let tiles = TileSet::new(root);
        let ocean = std::fs::read(tiles.ocean_path()).map_err(|e| format!("{}: {e}", tiles.ocean_path().display()))?;
        Ok(Baked {
            encoding: HeaderValue::from_str(encoding).map_err(|e| e.to_string())?,
            ocean: Bytes::from(ocean),
            tiles,
        })
    }

    fn serve(&self, packed: Bytes) -> Response {
        (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream")),
                (header::CONTENT_ENCODING, self.encoding.clone()),
            ],
            packed,
        )
            .into_response()
    }
}

struct App {
    /// Set when terrain is a finished bake rather than demand-paged.
    baked: Option<Baked>,
    /// No database behind this process: a bake is being served and none was
    /// there to take. What the database answers answers nothing.
    no_geo: bool,
    /// The coarsest level demand paging makes.
    top_level: u32,
    workers: Mutex<Vec<Worker>>,
    terrain_base: PathBuf,
    terrain: Mutex<Arc<Pager>>,
    poi: Mutex<HashMap<IVec, Cached>>,
    geocode: Mutex<HashMap<IVec, Cached>>,
    asset_dir: PathBuf,
    admin_token: Option<String>,
}

impl App {
    /// Takes a worker, runs `f` on it, and puts it back. Blocking by design:
    /// the whole bake is `Rc`-backed and synchronous, so it runs on a blocking
    /// thread and the async side only waits for it.
    ///
    /// None where there are no workers at all, which is a bake served with no
    /// database behind it: the wait below is for one that is busy, and a wait
    /// for one that does not exist is a thread that never comes back.
    fn with_worker<T>(&self, f: impl FnOnce(&mut Worker) -> T) -> Option<T> {
        if self.no_geo {
            return None;
        }
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

/// A leaf, baked on demand. A level above it is folded from every leaf under it,
/// which is the offline bake's work and not a request's, so this answers level
/// 0 alone and anything else is not here.
/// A stored object, labelled as the bucket labels it.
fn stored(packed: Bytes) -> Response {
    ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_ENCODING, CONTENT_ENCODING)], packed).into_response()
}

/// Settles a page: on disk already, or made now from what it depends on.
/// Boxed because it recurses — a level's fault faults the level below.
fn fault(app: Arc<App>, pager: Arc<Pager>, level: u32, page: IVec) -> Pin<Box<dyn Future<Output = Settled> + Send>> {
    Box::pin(async move {
        let cell = pager.faults.lock().expect("faults").entry((level, page)).or_default().clone();
        cell.get_or_init(|| settle(app, pager.clone(), level, page)).await.clone()
    })
}

async fn settle(app: Arc<App>, pager: Arc<Pager>, level: u32, page: IVec) -> Settled {
    if let Some(exists) = pager.tiles.settled(level, page) {
        return Ok(exists);
    }
    // No leaf under it, so nothing to fold and nothing to ask the database.
    if !pager.coverage.meets(level, page) {
        return Ok(false);
    }
    if level == 0 {
        return tokio::task::spawn_blocking(move || {
            app.with_worker(|w| {
                if !has_land(&mut w.client, &[page]).map_err(|e| e.to_string())?[0] {
                    pager.tiles.write_absent(0, page)?;
                    return Ok(false);
                }
                let (_, packed) = bake_leaf(&w.terrain, &mut w.client, page, &mut w.stats, Level::Live)?;
                pager.tiles.write(0, page, &packed)?;
                let n = pager.leaves.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n.is_multiple_of(500) {
                    println!("[tile/terrain] {n} leaves baked since the cache opened");
                }
                Ok(true)
            })
            // No worker is no database, and a leaf is baked from one: there
            // is nothing to bake and nothing was written.
            .unwrap_or(Ok(false))
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    let below: Vec<_> = neighbourhood(page, level).into_iter().map(|p| tokio::spawn(fault(app.clone(), pager.clone(), level - 1, p))).collect();
    for task in below {
        task.await.map_err(|e| e.to_string())??;
    }
    tokio::task::spawn_blocking(move || match fold_stored(&pager.tiles, page, level, &pager.ocean)? {
        None => {
            pager.tiles.write_absent(level, page)?;
            Ok(false)
        }
        Some(t) => {
            let raw = encode_terrain(&t.ele, &t.ground).map_err(|e| format!("{e:?}"))?;
            pager.tiles.write(level, page, &pack(&raw, Level::Live).map_err(|e| e.to_string())?)?;
            Ok(true)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A page of any level, served as the bucket serves it: the stored object,
/// packed, or a 404 the client reads as open water.
async fn terrain(State(app): State<Arc<App>>, Path((level, raw)): Path<(String, String)>) -> Response {
    let Some(level) = level.parse::<u32>().ok().filter(|l| *l <= PAGE_TOP_LEVEL) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(idx) = page_idx(&raw) else {
        return (StatusCode::BAD_REQUEST, "bad page_idx").into_response();
    };
    if let Some(baked) = &app.baked {
        return match tokio::fs::read(baked.tiles.path(level, idx)).await {
            Ok(packed) => {
                println!("[tile/terrain] {level}/{raw} {} B, baked", packed.len());
                baked.serve(Bytes::from(packed))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                println!("[tile/terrain] {level}/{raw} none (404), baked");
                StatusCode::NOT_FOUND.into_response()
            }
            Err(err) => {
                eprintln!("[tile/terrain/{level}/{raw}] {err}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }
    // Not a 404, which reads as open water: the page exists, it is only not
    // made here. The client counts it a failure and asks again later.
    if level > app.top_level {
        println!("[tile/terrain] {level}/{raw} past --top-level {} (503)", app.top_level);
        return (StatusCode::SERVICE_UNAVAILABLE, "past --top-level: serve a finished bake with --baked").into_response();
    }
    let pager = app.terrain.lock().expect("pager").clone();
    let cached = pager.tiles.settled(level, idx).is_some();
    let started = std::time::Instant::now();
    let log = |what: &str| {
        let how = if cached {
            "cached".to_string()
        } else {
            format!("made in {:.1}s", started.elapsed().as_secs_f64())
        };
        println!("[tile/terrain] {level}/{raw} {what}, {how}");
    };
    match fault(app, pager.clone(), level, idx).await {
        Ok(true) => match tokio::fs::read(pager.tiles.path(level, idx)).await {
            Ok(packed) => {
                log(&format!("{} B", packed.len()));
                stored(Bytes::from(packed))
            }
            Err(err) => {
                eprintln!("[tile/terrain/{level}/{raw}] {err}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Ok(false) => {
            log("none (404)");
            StatusCode::NOT_FOUND.into_response()
        }
        Err(err) => {
            eprintln!("[tile/terrain/{level}/{raw}] {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn ocean(State(app): State<Arc<App>>) -> Response {
    if let Some(baked) = &app.baked {
        return baked.serve(baked.ocean.clone());
    }
    stored(app.terrain.lock().expect("pager").ocean_packed.clone())
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
        .unwrap_or_else(|| Ok(Bytes::new()))
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
        app.with_worker(|w| w.geocode.reverse(&mut w.client, idx).map_err(|e| e.to_string())).unwrap_or(Ok(None)).map(|result| {
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
    // Terrain drops whole or not at all: a page is folded from the pages
    // under it, so dropping one would leave its ancestors from before.
    if kind == "terrain" {
        if app.baked.is_some() {
            return (StatusCode::CONFLICT, "serving a finished bake: nothing to invalidate").into_response();
        }
        if !body.trim().is_empty() {
            return (StatusCode::BAD_REQUEST, "terrain invalidates whole: send no page_idx").into_response();
        }
        let (old, generation, coverage) = {
            let pager = app.terrain.lock().expect("pager");
            (pager.tiles.root.clone(), pager.generation + 1, pager.coverage)
        };
        let base = app.terrain_base.clone();
        return match tokio::task::spawn_blocking(move || Pager::open(&base, generation, coverage)).await.expect("reopen") {
            Ok(fresh) => {
                *app.terrain.lock().expect("pager") = Arc::new(fresh);
                // Best effort: a fault still running against it may put a
                // directory back, which nothing reads and the next start drops.
                let _ = tokio::fs::remove_dir_all(old).await;
                (StatusCode::OK, "invalidated all").into_response()
            }
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        };
    }
    let cache = match kind.as_str() {
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
    let mut cache = PathBuf::from(format!("./out/dev/terrain/{}", PUBLISHED_VERSION.terrain));
    let mut top_level: u32 = 3;
    let mut baked = false;

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
            "--cache" => cache = PathBuf::from(next(&mut i)?),
            "--top-level" => top_level = next(&mut i)?.parse().map_err(|_| "bad --top-level")?,
            "--baked" => baked = true,
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }

    // Whatever the cache held was made by some earlier code against some
    // earlier data, so a start drops it whole.
    if cache.exists() {
        std::fs::remove_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    }

    let baked = if baked {
        let b = Baked::open(PathBuf::from(format!("./out/terrain/{}", PUBLISHED_VERSION.terrain)))?;
        println!("[tile] terrain: the finished bake at {}", b.tiles.root.display());
        Some(b)
    } else {
        None
    };

    // The database is what a leaf is baked from, so a bake it is serving
    // instead does not need one — and saying so is the difference between a
    // view's work needing Docker and a Postgres and needing neither. It is
    // still taken where it is there, since poi and geocode are the
    // database's whatever the terrain comes from; where it is not, those
    // answer that they have nothing and the terrain is served as it was.
    let url = std::env::var("DB_GEO_URL");
    let pool = match (&url, baked.is_some()) {
        (Err(_), false) => return Err("env:DB_GEO_URL:missing".into()),
        (Err(_), true) => Vec::new(),
        (Ok(url), served) => {
            let mut pool = Vec::with_capacity(workers.max(1));
            for _ in 0..workers.max(1) {
                match Worker::connect(url) {
                    Ok(w) => pool.push(w),
                    Err(e) if served => {
                        println!("[tile] geo: none ({e}); poi and geocode answer nothing");
                        break;
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            pool
        }
    };
    if !pool.is_empty() {
        println!("[tile] geo: ok ({} workers)", pool.len());
    }

    let app = Arc::new(App {
        baked,
        top_level,
        no_geo: pool.is_empty(),
        workers: Mutex::new(pool),
        terrain: Mutex::new(Arc::new(Pager::open(&cache, 0, Coverage::of_source())?)),
        terrain_base: cache,
        poi: Mutex::new(HashMap::new()),
        geocode: Mutex::new(HashMap::new()),
        asset_dir,
        // Absent in a dev process is not a hole: nothing outside the machine
        // reaches it, and requiring a token to clear a cache would only make
        // the loop it exists for harder to run.
        admin_token: std::env::var("ADMIN_TOKEN").ok().filter(|t| !t.is_empty()),
    });

    // The roots and branches are spec/pxl/spec/route.json's, as the client's and the
    // relays' are; what each one carries past them is this server's.
    let router = Router::new()
        .route(&format!("{TILE_ROOT}/terrain/_ocean"), get(ocean))
        .route(&format!("{TILE_ROOT}/terrain/{{level}}/{{page_idx}}"), get(terrain))
        .route(&format!("{TILE_ROOT}/poi/{{page_idx}}"), get(poi))
        .route(&format!("{TILE_ROOT}/geocode/{{page_idx}}"), get(geocode))
        // Its own root: an asset is neither server's, and its URL is its bucket
        // key with a leading slash.
        .route(&format!("{ASSET_ROOT}/{{hash}}"), get(asset))
        .route(&format!("{GEO_INTERNAL}/world/{{kind}}/cache/invalidate"), post(invalidate))
        .with_state(app.clone());

    // The workers' connections are the blocking client's, which drives a
    // runtime of its own when it is dropped, and dropping one inside this
    // runtime panics — mid-unwind, an abort that buries the error that ended
    // the server, a port already taken, under a stack of the drop's. So the
    // last hold on them is kept out here and let go once the runtime is gone.
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
    let served = runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind((host.as_str(), port)).await.map_err(|e| format!("{host}:{port}: {e}"))?;
        println!("[tile] listening on {host}:{port}");
        axum::serve(listener, router).await.map_err(|e| e.to_string())
    });
    drop(runtime);
    drop(app);
    served
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
