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
use pxl_spec::bake::constant::{LEVEL0_ZOOM, PAGE_TOP_LEVEL};
use pxl_spec::bake::format::{encode_poi_page, encode_terrain, Terrain};
use pxl_spec::bake::place_page::{encode_place_group, DICT_OBJECT, GROUP_PAGES, GROUP_SHAMT};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::bake::quadkey::index_of;
use pxl_spec::route::{ASSET_ROOT, GEO_INTERNAL, TILE_ROOT};
use pxl_spec::space::pvf3::ivec::{self, IVec};

use pxl_geo_bake::geocode::provider::overture::GeocodeProvider;
use pxl_geo_bake::geocode::PlaceDict;
use pxl_geo_bake::poi::provider::overture::PoiProvider;
use pxl_geo_bake::terrain::leaf::{bake_leaf, bake_ocean, has_land, ocean_samples, Coverage};
use pxl_geo_bake::terrain::pyramid::{fold_stored, neighbourhood};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::SourceStats;
use pxl_geo_bake::terrain::tile_set::{read_page, TileSet};
use pxl_geo_bake::util::{pack, Level, CONTENT_ENCODING};
use tokio::sync::OnceCell;

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

struct App {
    /// The coarsest level demand paging makes.
    top_level: u32,
    workers: Mutex<Vec<Worker>>,
    terrain_base: PathBuf,
    terrain: Mutex<Arc<Pager>>,
    poi: Mutex<HashMap<IVec, Cached>>,
    /// Keyed by group, which is what a reverse-geocode object is.
    geocode: Mutex<HashMap<IVec, Cached>>,
    /// The dictionary, read on the first ask and held. A group's numbers are
    /// places in it, so the one this process answers `_dict` with has to be
    /// the one it numbered its groups against.
    place_dict: Mutex<Option<Arc<PlaceDict>>>,
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
    /// The dictionary, read once. None when there is no database to read it
    /// from, which is what "geocode answers nothing" means here.
    fn place_dict(&self) -> Option<Result<Arc<PlaceDict>, String>> {
        if let Some(dict) = self.place_dict.lock().expect("dict").clone() {
            return Some(Ok(dict));
        }
        let read = self.with_worker(|w| w.geocode.divisions(&mut w.client).map_err(|e| e.to_string()))?;
        Some(read.map(|rows| {
            let dict = Arc::new(PlaceDict::of(rows));
            *self.place_dict.lock().expect("dict") = Some(dict.clone());
            dict
        }))
    }

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

/// The page a name addresses, if it is a name of the length this route serves.
///
/// There is no range check left to make. A page index used to arrive as a
/// decimal that had to be bounded before use — the page maths happily shifts
/// and unprojects a nonsense value into a well-formed bbox with no error
/// anywhere along the way, and that reached the database as a garbage query.
/// A quadkey of a given length is a page and there are no others, so the
/// length is the whole of the check (pxl-spec bake/quadkey.rs).
fn page_at(raw: &str, zoom: u32) -> Option<IVec> {
    index_of(raw).filter(|(z, _)| *z == zoom).map(|(_, idx)| idx)
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
async fn terrain(State(app): State<Arc<App>>, Path(raw): Path<String>) -> Response {
    // The name's length is the level, so one parse answers both and a level
    // past the pyramid's top is a name too short to be one.
    let Some((zoom, idx)) = index_of(&raw).filter(|(z, _)| LEVEL0_ZOOM - *z <= PAGE_TOP_LEVEL) else {
        return (StatusCode::BAD_REQUEST, "bad page name").into_response();
    };
    let level = LEVEL0_ZOOM - zoom;
    // Not a 404, which reads as open water: the page exists, it is only not
    // made here. The client counts it a failure and asks again later.
    if level > app.top_level {
        println!("[tile/terrain] {raw} past --top-level {} (503)", app.top_level);
        return (StatusCode::SERVICE_UNAVAILABLE, "past --top-level: raise it, or read the published set with dev:remote").into_response();
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
        println!("[tile/terrain] {raw} {what}, {how}");
    };
    match fault(app, pager.clone(), level, idx).await {
        Ok(true) => match tokio::fs::read(pager.tiles.path(level, idx)).await {
            Ok(packed) => {
                log(&format!("{} B", packed.len()));
                stored(Bytes::from(packed))
            }
            Err(err) => {
                eprintln!("[tile/terrain/{raw}] {err}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        },
        Ok(false) => {
            log("none (404)");
            StatusCode::NOT_FOUND.into_response()
        }
        Err(err) => {
            eprintln!("[tile/terrain/{raw}] {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn ocean(State(app): State<Arc<App>>) -> Response {
    stored(app.terrain.lock().expect("pager").ocean_packed.clone())
}

async fn poi(State(app): State<Arc<App>>, Path(raw): Path<String>) -> Response {
    // POI is baked at the leaf and nowhere else.
    let Some(idx) = page_at(&raw, LEVEL0_ZOOM) else {
        return (StatusCode::BAD_REQUEST, "bad page name").into_response();
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

/// Every page under a group, in slot order. The bake asks only about the land
/// ones, from terrain's list; this has no such list and asks about all of
/// them, which costs a query a page on the first request for a group and
/// nothing after it.
fn group_leaves(group: IVec) -> Vec<IVec> {
    let base = ivec::sh(group, GROUP_SHAMT as i32);
    let side = 1i64 << GROUP_SHAMT;
    (0..GROUP_PAGES as i64).map(|k| ivec::add(base, ivec::gather(k % side, k / side, 0))).collect()
}

async fn geocode(State(app): State<Arc<App>>, Path(raw): Path<String>) -> Response {
    if raw == DICT_OBJECT {
        let read = tokio::task::spawn_blocking(move || app.place_dict()).await.expect("geocode dict task");
        return match read {
            None => (StatusCode::NOT_FOUND, "not found").into_response(),
            Some(Ok(dict)) => match dict.packet() {
                Ok(pkt) => octet_stream(Bytes::from(pkt)),
                Err(err) => {
                    eprintln!("[tile/geocode/_dict] {err:?}");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            },
            Some(Err(err)) => {
                eprintln!("[tile/geocode/_dict] {err}");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }

    // A group covers GROUP_SHAMT levels of pages, so its name is that much
    // shorter than a page's.
    let Some(idx) = page_at(&raw, LEVEL0_ZOOM - GROUP_SHAMT) else {
        return (StatusCode::BAD_REQUEST, "bad group name").into_response();
    };

    match app.geocode.lock().expect("cache").get(&idx).cloned() {
        Some(Cached::Bytes(b)) => return octet_stream(b),
        // A group no page of which is in any place has no object, and the
        // client reads the 404 as "no label" rather than as an error.
        Some(Cached::Absent) => return (StatusCode::NOT_FOUND, "not found").into_response(),
        _ => {}
    }

    let made = tokio::task::spawn_blocking(move || {
        let dict = match app.place_dict() {
            None => return Ok(Cached::Absent),
            Some(read) => read?,
        };
        let group = app.with_worker(|w| w.geocode.group(&mut w.client, &dict, &group_leaves(idx))).unwrap_or(Ok(None))?;
        let cached = match group {
            Some(group) => Cached::Bytes(Bytes::from(encode_place_group(&group).map_err(|e| format!("{e:?}"))?)),
            None => Cached::Absent,
        };
        app.geocode.lock().expect("cache").insert(idx, cached.clone());
        Ok::<Cached, String>(cached)
    })
    .await
    .expect("geocode task");

    match made {
        Ok(Cached::Bytes(b)) => octet_stream(b),
        Ok(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(err) => {
            eprintln!("[tile/geocode/{raw}] {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// A key is the sha256 of the object's bytes; the ingest writes each under its
/// type, so the content type is known without an extension.
/// What an asset's extension says it is. The bucket stores the type with the
/// object; here it is read off the name, which is the same fact either way.
const ASSET_TYPES: [(&str, &str); 3] = [("glb", "model/gltf-binary"), ("png", "image/png"), ("webp", "image/webp")];

async fn asset(State(app): State<Arc<App>>, Path(name): Path<String>) -> Response {
    // An asset's name is its own path, so a request could otherwise ask for a
    // path out of the ingest's tree.
    if name.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    let Some(content_type) = name.rsplit_once('.').and_then(|(_, ext)| ASSET_TYPES.iter().find(|(e, _)| *e == ext)).map(|(_, t)| *t) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    match tokio::fs::read(app.asset_dir.join(&name)).await {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"));
            (headers, bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found — run the asset ingest").into_response(),
    }
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
    // Each bake names its objects at its own zoom, so which one is being
    // invalidated says how long a name to expect.
    let (cache, zoom) = match kind.as_str() {
        "poi" => (&app.poi, LEVEL0_ZOOM),
        "geocode" => (&app.geocode, LEVEL0_ZOOM - GROUP_SHAMT),
        _ => return (StatusCode::NOT_FOUND, "unknown cache").into_response(),
    };

    let one = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("name").and_then(|p| p.as_str().map(str::to_string)));
    match one {
        Some(raw) => match page_at(&raw, zoom) {
            Some(idx) => {
                cache.lock().expect("cache").remove(&idx);
                (StatusCode::OK, format!("invalidated {raw}")).into_response()
            }
            None => (StatusCode::BAD_REQUEST, "bad page name").into_response(),
        },
        None => {
            if kind == "geocode" {
                *app.place_dict.lock().expect("dict") = None;
            }
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
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }

    // Whatever the cache held was made by some earlier code against some
    // earlier data, so a start drops it whole.
    if cache.exists() {
        std::fs::remove_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    }

    // Every page this serves is made from the database, so there is no
    // running without one.
    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let mut pool = Vec::with_capacity(workers.max(1));
    for _ in 0..workers.max(1) {
        pool.push(Worker::connect(&url).map_err(|e| e.to_string())?);
    }
    println!("[tile] geo: ok ({} workers)", pool.len());

    let app = Arc::new(App {
        top_level,
        workers: Mutex::new(pool),
        terrain: Mutex::new(Arc::new(Pager::open(&cache, 0, Coverage::of_source())?)),
        terrain_base: cache,
        poi: Mutex::new(HashMap::new()),
        geocode: Mutex::new(HashMap::new()),
        place_dict: Mutex::new(None),
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
        .route(&format!("{TILE_ROOT}/terrain/{{quadkey}}"), get(terrain))
        .route(&format!("{TILE_ROOT}/poi/{{quadkey}}"), get(poi))
        .route(&format!("{TILE_ROOT}/geocode/{{quadkey}}"), get(geocode))
        // Its own root: an asset is neither server's, and its URL is its bucket
        // key with a leading slash.
        .route(&format!("{ASSET_ROOT}/{{*name}}"), get(asset))
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
