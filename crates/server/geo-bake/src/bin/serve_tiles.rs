// pxl-geo-bake/bin/serve_tiles.rs

//! Local demand bakes. --cache holds the terrain/poi/geocode directories;
//! a tile stays cached across restarts until its kind is invalidated. The
//! Earth's picture is baked offline by pxl-tool, so it is served from that
//! bake (`--out`), under the names the bucket holds, whichever way the rest is.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::demand::{
    job::{self, Limits},
    Failure, Kind, Server,
};
use pxl_geo_bake::pyramid::TileSet;
use pxl_geo_bake::util::CONTENT_ENCODING;
use pxl_server::admin_auth::admin_auth;
use pxl_spec::bake::published::{PUBLISHED_TOP_ZOOM, PUBLISHED_VERSION};
use pxl_spec::bake::quadkey::tile_of_name;
use pxl_spec::route::{ASSET_ROOT, GEO_INTERNAL, TILE_ROOT};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

enum Tiles {
    Demand(Server),
    Baked(PathBuf),
}

struct App {
    tiles: Tiles,
    top_z: Zoom,
    asset_dir: PathBuf,
    out: PathBuf,
}

const EARTH: &str = "earth";

async fn earth(app: &App, version: &str, name: &str) -> Response {
    if tile_of_name(name).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(app.out.join(EARTH).join(version).join(name)).await {
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

async fn tile(State(app): State<Arc<App>>, Path((kind, name)): Path<(String, String)>) -> Response {
    if kind == EARTH {
        return earth(&app, PUBLISHED_VERSION.earth, &name).await;
    }
    let Some(kind) = Kind::parse(&kind) else { return StatusCode::NOT_FOUND.into_response() };
    let demand = match &app.tiles {
        Tiles::Demand(demand) => demand,
        Tiles::Baked(dir) => return baked(dir, kind, published_version(kind), &name).await,
    };
    match demand.get(kind, &name).await {
        Ok(Some(bytes)) => packed(bytes),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(Failure::BadName) => (StatusCode::BAD_REQUEST, "bad tile name").into_response(),
        Err(Failure::Unavailable) => (StatusCode::SERVICE_UNAVAILABLE, "past --folds").into_response(),
        Err(Failure::Limited(reason)) => (StatusCode::SERVICE_UNAVAILABLE, reason).into_response(),
        Err(Failure::Bake(e)) => {
            eprintln!("[tile/{name}] {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn published_version(kind: Kind) -> &'static str {
    match kind {
        Kind::Terrain => PUBLISHED_VERSION.terrain,
        Kind::Poi => PUBLISHED_VERSION.poi,
        Kind::Geocode => PUBLISHED_VERSION.geocode,
    }
}

/// The bucket's address, `<kind>/<version>/<name>`, as a production build asks for it.
async fn tile_versioned(State(app): State<Arc<App>>, Path((kind, version, name)): Path<(String, String, String)>) -> Response {
    // `version_name`'s shape, v<format>.<day>.<n>: no other name reaches the disk
    if !version
        .strip_prefix('v')
        .is_some_and(|n| n.bytes().next().is_some_and(|b| b.is_ascii_digit()) && n.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b'-'))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    if kind == EARTH {
        return earth(&app, &version, &name).await;
    }
    let (Tiles::Baked(dir), Some(kind)) = (&app.tiles, Kind::parse(&kind)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    baked(dir, kind, &version, &name).await
}

async fn baked(dir: &std::path::Path, kind: Kind, version: &str, name: &str) -> Response {
    let root = dir.join(kind.name()).join(version);
    // A pyramid's tiles are on disk as `z<zoom>/<tile index>` until the publish renames them.
    let path = match kind {
        Kind::Terrain | Kind::Poi => match tile_of_name(name) {
            Some(p) => TileSet::new(root).path(p),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
        Kind::Geocode if name != "." && name != ".." => root.join(name),
        Kind::Geocode => return StatusCode::NOT_FOUND.into_response(),
    };
    match tokio::fs::read(path).await {
        Ok(bytes) => packed(bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn source(State(app): State<Arc<App>>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-store")],
        serde_json::json!({ "top_z": app.top_z }).to_string(),
    )
        .into_response()
}

async fn invalidate(State(app): State<Arc<App>>, Path(kind): Path<String>, body: String) -> Response {
    let Some(kind) = Kind::parse(&kind) else { return StatusCode::NOT_FOUND.into_response() };
    let Tiles::Demand(demand) = &app.tiles else {
        return (StatusCode::CONFLICT, "a finished bake is served as it is: nothing to invalidate").into_response();
    };
    if !body.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "invalidate the whole dataset: send no body").into_response();
    }
    match demand.invalidate(kind) {
        Ok(()) => (StatusCode::OK, "invalidated all").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
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
    let mut cache = PathBuf::from("./cache/tile");
    // Zooms folded over the leaves: a cold tile at the top bakes 4^folds leaves.
    let mut folds: Zoom = 2;
    let mut baked: Option<PathBuf> = None;
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
            "--baked" => baked = Some(PathBuf::from(flags.value()?)),
            "--out" => out = PathBuf::from(flags.value()?),
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let (tiles, top_z) = match baked {
        Some(dir) => (Tiles::Baked(dir), PUBLISHED_TOP_ZOOM),
        None => {
            let top_z = LEAF_ZOOM.checked_sub(folds).filter(|&z| z >= GROUND.top()).ok_or("--folds past the pyramid's top")?;
            (Tiles::Demand(Server::open(workers, cache, top_z, limits)?), top_z)
        }
    };
    let app = Arc::new(App { tiles, top_z, asset_dir, out });

    // Without ADMIN_TOKEN, as in local dev, the internal route is open.
    let mut internal = Router::new().route(&format!("{GEO_INTERNAL}/world/{{kind}}/cache/invalidate"), post(invalidate));
    if let Some(token) = std::env::var("ADMIN_TOKEN").ok().filter(|t| !t.is_empty()) {
        internal = internal.route_layer(axum::middleware::from_fn_with_state(token, admin_auth));
    }
    let router = Router::new()
        .route(&format!("{TILE_ROOT}/source"), get(source))
        .route(&format!("{TILE_ROOT}/{{kind}}/{{name}}"), get(tile))
        .route(&format!("{TILE_ROOT}/{{kind}}/{{version}}/{{name}}"), get(tile_versioned))
        .route(&format!("{ASSET_ROOT}/{{*name}}"), get(asset))
        .merge(internal)
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
