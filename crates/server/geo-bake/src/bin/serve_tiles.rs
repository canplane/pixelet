// pxl-geo-bake/bin/serve_tiles.rs

//! Local demand bakes. --cache holds them a kind a directory, across
//! restarts; a re-ingest or a bake change is a purge by command. The
//! Earth's picture is baked offline by pxl-tool, so it is served from that
//! bake (`--out`), under the names the bucket holds, whichever way the rest is.
//! `--shelf` serves each kind's set (tile/shard.rs), so a bake is looked at
//! before anything is published. A tile goes out immutable under a version
//! that names what is served, as the Worker's do: a set's, its manifest's
//! digest; the oven's, the token its cache holds (`version_of`).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::Coverage;
use pxl_geo_bake::demand::{
    job::{self, Limits},
    Failure, Kind, Server,
};
use pxl_geo_bake::source::{self, Overrides};
use pxl_geo_bake::util::CONTENT_ENCODING;
use pxl_spec::bake::published::{BaseLayer, FORMAT};
use pxl_spec::bake::source::sharded;
use pxl_spec::route::{tile_object_parts, ASSET_ROOT, TILE_ROOT, TILE_SPHERE};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM, TOP_ZOOM};
use std::path::{Path as FsPath, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

/// Where the tiles come from: the oven, baked on demand, or the shelf, each
/// kind's set under a directory.
enum Tiles {
    Oven(Server),
    Shelf(PathBuf),
}

struct App {
    tiles: Tiles,
    /// The oven's cache, whose token names what it serves.
    cache: PathBuf,
    top_zoom: Zoom,
    coverage: Vec<String>,
    asset_dir: PathBuf,
    out: PathBuf,
}

/// The token an oven's cache holds, made with the directory, so a purge of
/// it is a new version.
const OVEN_TOKEN: &str = "version";

/// A version named by bytes that change whenever what is served does.
fn named(bytes: &[u8]) -> String {
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, bytes);
    format!("dev-{:016x}", std::hash::Hasher::finish(&h))
}

/// What a kind is served under now: a set's, its manifest's digest, which
/// every bake writes again; the oven's, its cache's token.
fn version_of(app: &App, layer: BaseLayer) -> Result<String, String> {
    let read = |path: &FsPath| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()));
    match &app.tiles {
        _ if !sharded(layer) => read(&app.out.join(EARTH).join(pxl_geo_bake::pyramid::MANIFEST)).map(|b| named(&b)),
        Tiles::Shelf(dir) => read(&dir.join(layer.name()).join(pxl_geo_bake::pyramid::MANIFEST)).map(|b| named(&b)),
        Tiles::Oven(_) => {
            let token = app.cache.join(OVEN_TOKEN);
            if !token.exists() {
                std::fs::create_dir_all(&app.cache).map_err(|e| format!("{}: {e}", app.cache.display()))?;
                let made = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
                std::fs::write(&token, made.to_string()).map_err(|e| format!("{}: {e}", token.display()))?;
            }
            read(&token).map(|b| named(&b))
        }
    }
}

/// The source's declaration (pxl-geo-bake source.rs) as the server stands
/// now: a shelf's sets as their manifests say, the oven the coverage in the
/// formats this code writes; the Earth's picture its bake's.
fn declaration(app: &App) -> Result<String, String> {
    let kinds = source::try_kinds(|l| {
        let version = version_of(app, l)?;
        match &app.tiles {
            _ if !sharded(l) => Ok(source::Kind {
                shard: None,
                ..source::of_set(&app.out.join(EARTH), &version)?
            }),
            Tiles::Shelf(dir) => match source::of_set(&dir.join(l.name()), &version)? {
                k if k.shard.is_some() => Ok(k),
                _ => Err(format!("{}: its manifest names no shards the set holds", dir.join(l.name()).display())),
            },
            Tiles::Oven(_) => Ok(source::Kind {
                format: FORMAT.of(l),
                version,
                shard: Some(source::tiles(app.coverage.iter().map(String::as_str))?),
            }),
        }
    })?;
    Ok(source::declaration(app.top_zoom, &kinds, &Overrides::new())?.to_string())
}

const EARTH: &str = "earth";

async fn earth(app: &App, name: &str) -> Response {
    if Tile::of_name(name).is_none() && name != TILE_SPHERE {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(app.out.join(EARTH).join(name)).await {
        Ok(bytes) => packed(bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Immutable, as the Worker serves a tile: its version names its bytes.
fn packed(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CONTENT_ENCODING, CONTENT_ENCODING),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable, no-transform"),
        ],
        Bytes::from(bytes),
    )
        .into_response()
}

async fn tile(State(app): State<Arc<App>>, Path((kind, object)): Path<(String, String)>) -> Response {
    let Some((version, name)) = tile_object_parts(&object) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Only what is served now, so nothing is kept under a version that named other bytes.
    let layer = BaseLayer::ALL.into_iter().find(|l| l.name() == kind);
    if layer.is_none_or(|l| version_of(&app, l).ok().as_deref() != Some(version)) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let name = name.to_string();
    if kind == EARTH {
        return earth(&app, &name).await;
    }
    let Some(kind) = Kind::parse(&kind) else { return StatusCode::NOT_FOUND.into_response() };
    let demand = match &app.tiles {
        Tiles::Oven(demand) => demand,
        Tiles::Shelf(dir) => return from_shelf(dir, kind, &name).await,
    };
    match demand.get(kind, &name).await {
        Ok(Some(bytes)) => packed(bytes),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(Failure::BadName) => (StatusCode::BAD_REQUEST, "bad tile name").into_response(),
        Err(Failure::Unavailable) => (StatusCode::SERVICE_UNAVAILABLE, "past --folds").into_response(),
        Err(Failure::Limited(reason)) => (StatusCode::SERVICE_UNAVAILABLE, reason).into_response(),
        Err(Failure::Bake(e)) => {
            eprintln!("[tile/{name}] {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn from_shelf(dir: &std::path::Path, kind: Kind, name: &str) -> Response {
    // Only a tile's name reaches the disk; a set's tiles lie flat under them.
    if Tile::of_name(name).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let set = dir.join(kind.name());
    match tokio::fs::read(set.join(name)).await {
        Ok(bytes) => packed(bytes),
        // A page the set has no object for is empty, as the Worker answers it.
        Err(_) if set.is_dir() => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Made again at each ask, so a set baked again under a running server is a new version.
async fn source(State(app): State<Arc<App>>) -> Response {
    match declaration(&app) {
        Ok(declared) => ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-store")], declared).into_response(),
        Err(e) => {
            eprintln!("[tile] {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

const ASSET_TYPES: [(&str, &str); 3] = [("glb", "model/gltf-binary"), ("png", "image/png"), ("webp", "image/webp")];

async fn asset(State(app): State<Arc<App>>, Path(name): Path<String>) -> Response {
    // Refuse a path out of the ingest's tree.
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

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().is_some_and(|a| a == "--demand-job") {
        if argv.len() != 4 {
            return Err("invalid private demand job".into());
        }
        let job = serde_json::from_str(&argv[1]).map_err(|e| e.to_string())?;
        let seconds = argv[3].parse().map_err(|_| "invalid job deadline")?;
        return job::execute(job, std::path::Path::new(&argv[2]), seconds);
    }
    let mut limits = Limits::default();
    let mut port: u16 = 3001;
    let mut host = "127.0.0.1".to_string();
    let mut asset_dir = PathBuf::from("./out/asset");
    let mut workers: usize = 4;
    let mut cache = PathBuf::from("./out/oven");
    // Zooms folded over the leaves: a cold tile at the top bakes 4^folds leaves.
    let mut folds: Zoom = 2;
    let mut shelf: Option<PathBuf> = None;
    let mut out = PathBuf::from("./out");

    let mut flags = Flags::of(argv);
    while let Some(flag) = flags.arg() {
        match flag.as_str() {
            "--port" => port = flags.parse()?,
            "--host" => host = flags.value()?,
            "--asset" => asset_dir = PathBuf::from(flags.value()?),
            "--queue" => limits.queued = flags.parse()?,
            "--job-seconds" => limits.seconds = flags.parse()?,
            "--workers" => workers = flags.parse()?,
            "--cache" => cache = PathBuf::from(flags.value()?),
            "--folds" => folds = flags.parse()?,
            "--shelf" => shelf = Some(PathBuf::from(flags.value()?)),
            "--out" => out = PathBuf::from(flags.value()?),
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let (tiles, top_zoom) = match shelf {
        Some(dir) => (Tiles::Shelf(dir), TOP_ZOOM),
        None => {
            let top_zoom = LEAF_ZOOM.checked_sub(folds).filter(|&z| z >= TOP_ZOOM).ok_or("--folds past the pyramid's top")?;
            (Tiles::Oven(Server::open(workers, cache.clone(), top_zoom, limits)?), top_zoom)
        }
    };
    let coverage: Vec<String> = Coverage::of_source()?.shards().map(|s| s.name()).collect();
    let app = Arc::new(App {
        tiles,
        cache: cache.clone(),
        top_zoom,
        coverage,
        asset_dir,
        out,
    });
    // Refused at start as a client would refuse it.
    declaration(&app)?;

    let router = Router::new()
        .route(&format!("{TILE_ROOT}/source"), get(source))
        .route(&format!("{TILE_ROOT}/{{kind}}/{{*object}}"), get(tile))
        .route(&format!("{ASSET_ROOT}/{{*name}}"), get(asset))
        .with_state(app.clone());

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
