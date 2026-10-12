// pxl-geo-bake/bin/serve_tiles.rs

//! Local demand bakes. --cache holds them a kind a directory, across
//! restarts; a re-ingest or a bake change is a purge by command. The
//! Earth's picture is baked offline by pxl-tool, so it is served from that
//! bake (`--out`), under the names the bucket holds, whichever way the rest is.
//! `--shelf` serves each kind's set (tile/shard.rs), so a bake is looked at
//! before anything is published.

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
use pxl_spec::bake::published::FORMAT;
use pxl_spec::bake::source::sharded;
use pxl_spec::route::{tile_object_parts, ASSET_ROOT, TILE_ROOT, TILE_SPHERE};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

/// Where the tiles come from: the oven, baked on demand, or the shelf, each
/// kind's set under a directory.
enum Tiles {
    Oven(Server),
    Shelf(PathBuf),
}

/// The version it declares its tiles under: it serves one thing, so the
/// version names nothing and is never read.
const VERSION: &str = "dev";

struct App {
    tiles: Tiles,
    /// The source's declaration (pxl-geo-bake source.rs).
    source: String,
    asset_dir: PathBuf,
    out: PathBuf,
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

fn packed(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CONTENT_ENCODING, CONTENT_ENCODING),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Bytes::from(bytes),
    )
        .into_response()
}

async fn tile(State(app): State<Arc<App>>, Path((kind, object)): Path<(String, String)>) -> Response {
    let Some((_, name)) = tile_object_parts(&object) else {
        return StatusCode::NOT_FOUND.into_response();
    };
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

async fn source(State(app): State<Arc<App>>) -> Response {
    ([(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-store")], app.source.clone()).into_response()
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
    let mut cache = PathBuf::from("./cache/tile");
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
        Some(dir) => (Tiles::Shelf(dir), GROUND.top()),
        None => {
            let top_zoom = LEAF_ZOOM.checked_sub(folds).filter(|&z| z >= GROUND.top()).ok_or("--folds past the pyramid's top")?;
            (Tiles::Oven(Server::open(workers, cache, top_zoom, limits)?), top_zoom)
        }
    };
    // What it serves: a shelf's sets as their manifests say, the oven the
    // coverage in the formats this code writes; the Earth's picture its bake's.
    let coverage: Vec<String> = Coverage::of_source()?.shards().map(|s| s.name()).collect();
    let kinds = source::try_kinds(|l| match &tiles {
        _ if !sharded(l) => Ok(source::Kind {
            shard: None,
            ..source::of_set(&out.join(EARTH), VERSION)?
        }),
        Tiles::Shelf(dir) => match source::of_set(&dir.join(l.name()), VERSION)? {
            k if k.shard.is_some() => Ok(k),
            _ => Err(format!("{}: its manifest names no shards the set holds", dir.join(l.name()).display())),
        },
        Tiles::Oven(_) => Ok(source::Kind {
            format: FORMAT.of(l),
            version: VERSION.into(),
            shard: Some(source::tiles(coverage.iter().map(String::as_str))?),
        }),
    })?;
    let declared = source::declaration(top_zoom, &kinds, &Overrides::new())?.to_string();
    let app = Arc::new(App {
        tiles,
        source: declared,
        asset_dir,
        out,
    });

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
