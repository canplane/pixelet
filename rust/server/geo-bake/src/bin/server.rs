// pxl-geo-bake/bin/server.rs

//! The dev tile server, baking on demand. It keeps what it bakes, one directory a kind, across
//! restarts, in `--oven` or else in the checkout's `tmp/`. After a re-ingest or a change to a
//! bake the oven is purged by command. The Earth's picture is baked offline (`bin/earth.rs`),
//! so it is served from that bake (`./out`, or `--shelf`), under the names the bucket holds,
//! however the rest is served.
//! `--shelf` serves each kind's set (`tile/shard.rs`), so a bake can be looked at before
//! anything is published. A tile goes out immutable under a version that names what is
//! served, as the Worker's tiles do: a set's version is its manifest's digest, and the oven's
//! is the token its folder holds (`version_of`).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use pxl_geo_bake::cli;
use pxl_geo_bake::cli::Argv;
use pxl_geo_bake::coverage::Coverage;
use pxl_geo_bake::oven::{
    job::{self, Limit},
    Failure, Server,
};
use pxl_geo_bake::pyramid;
use pxl_geo_bake::source::{self, OverrideMap};
use pxl_geo_bake::util::CONTENT_ENCODING;
use pxl_spec::bake::publication::{tile_object_parts, BaseKind, FORMAT, TILE_SPHERE};
use pxl_spec::bake::source::is_sharded;
use pxl_spec::directory;
use pxl_spec::route::{ASSET_ROOT, TILE_ROOT};
use pxl_spec::space::coord::zoom::{Zoom, LEAF_ZOOM, MIN_ZOOM};
use pxl_spec::space::layer::quadrant::Quadrant;
use std::path::{Path as FsPath, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use tokio::fs;
use tokio::net::TcpListener;
use tokio::runtime::Builder;

/// Where the tiles come from: the oven, baked on demand, or the shelf, each
/// kind's set under a directory.
enum TileSource {
    Oven(Server),
    Shelf(PathBuf),
}

struct App {
    tile_source: TileSource,
    /// Where the oven keeps what it bakes, whose token names what it serves.
    oven: PathBuf,
    min_zoom: Zoom,
    coverage: Vec<String>,
    asset_dir: PathBuf,
    out: PathBuf,
}

/// The token an oven's folder holds, made with the folder, so a purge of
/// it is a new version.
const OVEN_TOKEN: &str = "version";

/// A version named by bytes that change whenever what is served does.
fn bytes_version(bytes: &[u8]) -> String {
    let mut h = fnv::FnvHasher::default();
    std::hash::Hasher::write(&mut h, bytes);
    format!("dev-{:016x}", std::hash::Hasher::finish(&h))
}

/// The version a kind is served under now: for a set, its manifest's digest, which every bake
/// writes again; for the oven, its folder's token.
fn version_of(app: &App, layer: BaseKind) -> Result<String, String> {
    let read = |path: &FsPath| std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()));
    match &app.tile_source {
        _ if !is_sharded(layer) => {
            read(&app.out.join(BaseKind::Earth.name()).join(pyramid::MANIFEST)).map(|b| bytes_version(&b))
        }
        TileSource::Shelf(dir) => {
            read(&dir.join(layer.name()).join(pyramid::MANIFEST)).map(|b| bytes_version(&b))
        }
        TileSource::Oven(_) => {
            let token = app.oven.join(OVEN_TOKEN);
            if !token.exists() {
                std::fs::create_dir_all(&app.oven)
                    .map_err(|e| format!("{}: {e}", app.oven.display()))?;
                let made = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos());
                std::fs::write(&token, made.to_string())
                    .map_err(|e| format!("{}: {e}", token.display()))?;
            }
            read(&token).map(|b| bytes_version(&b))
        }
    }
}

/// The source's declaration (`source.rs`) for the server as it stands: a shelf's
/// sets as their manifests say; for the oven, the coverage in the formats this code writes;
/// and for the Earth's picture, its bake's.
fn decl(app: &App) -> Result<String, String> {
    let kinds = source::try_kinds(|l| {
        let version = version_of(app, l)?;
        match &app.tile_source {
            _ if !is_sharded(l) => Ok(source::Kind {
                shard: None,
                ..source::of_set(&app.out.join(BaseKind::Earth.name()), &version)?
            }),
            TileSource::Shelf(dir) => match source::of_set(&dir.join(l.name()), &version)? {
                k if k.shard.is_some() => Ok(k),
                _ => Err(format!(
                    "{}: its manifest names no shards the set holds",
                    dir.join(l.name()).display()
                )),
            },
            TileSource::Oven(_) => Ok(source::Kind {
                format: FORMAT[l as usize],
                version,
                shard: Some(source::tiles(app.coverage.iter().map(String::as_str))?),
            }),
        }
    })?;
    Ok(source::decl(app.min_zoom, &kinds, &OverrideMap::new())?.to_string())
}

async fn earth(app: &App, name: &str) -> Response {
    if Quadrant::of_key(name).is_none() && name != TILE_SPHERE {
        return StatusCode::NOT_FOUND.into_response();
    }
    match fs::read(app.out.join(BaseKind::Earth.name()).join(name)).await {
        Ok(bytes) => tile_response(bytes),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Immutable, as the Worker serves a tile: its version names its bytes.
fn tile_response(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CONTENT_ENCODING, CONTENT_ENCODING),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable, no-transform",
            ),
        ],
        Bytes::from(bytes),
    )
        .into_response()
}

async fn tile(
    State(app): State<Arc<App>>,
    Path((kind, object)): Path<(String, String)>,
) -> Response {
    let Some((version, name)) = tile_object_parts(&object) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Only what is served now is answered, so nothing is kept under a version that named other
    // bytes.
    let Some(layer) = BaseKind::ALL.into_iter().find(|l| l.name() == kind) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if version_of(&app, layer).ok().as_deref() != Some(version) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let name = name.to_string();
    if !is_sharded(layer) {
        return earth(&app, &name).await;
    }
    let oven = match &app.tile_source {
        TileSource::Oven(oven) => oven,
        TileSource::Shelf(dir) => return from_shelf(dir, layer, &name).await,
    };
    match oven.get(layer, &name).await {
        Ok(Some(bytes)) => tile_response(bytes),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(Failure::BadName) => (StatusCode::BAD_REQUEST, "bad tile name").into_response(),
        Err(Failure::Unavailable) => {
            (StatusCode::SERVICE_UNAVAILABLE, "past --folds").into_response()
        }
        Err(Failure::Limited(reason)) => (StatusCode::SERVICE_UNAVAILABLE, reason).into_response(),
        Err(Failure::Bake(e)) => {
            eprintln!("[tile/{name}] {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn from_shelf(dir: &std::path::Path, kind: BaseKind, name: &str) -> Response {
    // Only a tile's name reaches the disk; a set's tiles lie flat under them.
    if Quadrant::of_key(name).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let set = dir.join(kind.name());
    match fs::read(set.join(name)).await {
        Ok(bytes) => tile_response(bytes),
        // A page the set has no object for is empty, as the Worker answers it.
        Err(_) if set.is_dir() => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Made again at each ask, so a set baked again under a running server is a new version.
async fn src(State(app): State<Arc<App>>) -> Response {
    match decl(&app) {
        Ok(declared) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            declared,
        )
            .into_response(),
        Err(e) => {
            eprintln!("[tile] {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

const ASSET_TYPES: [(&str, &str); 3] = [
    ("glb", "model/gltf-binary"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

async fn asset(State(app): State<Arc<App>>, Path(name): Path<String>) -> Response {
    // Refuse a path out of the ingest's tree.
    if name
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    let Some(content_type) = name
        .rsplit_once('.')
        .and_then(|(_, ext)| ASSET_TYPES.iter().find(|(e, _)| *e == ext))
        .map(|(_, t)| *t)
    else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    match fs::read(app.asset_dir.join(&name)).await {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            (headers, bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "not found — run the asset ingest").into_response(),
    }
}

fn run() -> Result<(), String> {
    cli::enter()?;
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().is_some_and(|a| a == "--oven-job") {
        if argv.len() != 4 {
            return Err("invalid private oven job".into());
        }
        let job = serde_json::from_str(&argv[1]).map_err(|e| e.to_string())?;
        let seconds = argv[3].parse().map_err(|_| "invalid job deadline")?;
        return job::execute(job, std::path::Path::new(&argv[2]), seconds);
    }
    let mut limit = Limit::default();
    let mut port: u16 = 3001;
    let mut host = "127.0.0.1".to_string();
    let mut asset_dir = None;
    let mut worker_cnt: usize = 4;
    let mut oven = None;
    // Zooms folded over the leaves: a cold tile at the top bakes 4^folds leaves.
    let mut fold_cnt: Zoom = 2;
    let mut shelf: Option<PathBuf> = None;

    let mut flags = Argv::of(argv);
    let mut oven_named = false;
    while let Some(flag) = flags.arg() {
        oven_named |= flag.starts_with("--oven");
        match flag.as_str() {
            "--port" => port = flags.parse()?,
            "--host" => host = flags.val()?,
            "--asset" => asset_dir = Some(PathBuf::from(flags.val()?)),
            "--oven" => oven = Some(PathBuf::from(flags.val()?)),
            "--oven-queue" => limit.queued = flags.parse()?,
            "--oven-job-seconds" => limit.seconds = flags.parse()?,
            "--oven-workers" => worker_cnt = flags.parse()?,
            "--oven-folds" => fold_cnt = flags.parse()?,
            "--shelf" => shelf = Some(PathBuf::from(flags.val()?)),
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    if oven_named && shelf.is_some() {
        return Err("--shelf serves no oven, so the --oven flags do not apply".into());
    }

    // A shelf's server has no oven, so it names no folder for one.
    let oven = match (oven, &shelf) {
        (None, None) => directory::tmp_dir("oven"),
        (oven, _) => oven.unwrap_or_default(),
    };
    // The folder the bakes write to: everything the server reads from it follows `--shelf`.
    let out = shelf.clone().unwrap_or_else(|| PathBuf::from("./out"));
    let asset_dir = asset_dir.unwrap_or_else(|| out.join("asset"));
    let (tile_source, min_zoom) = match shelf {
        Some(dir) => (TileSource::Shelf(dir), MIN_ZOOM),
        None => {
            let min_zoom = LEAF_ZOOM
                .checked_sub(fold_cnt)
                .filter(|&z| z >= MIN_ZOOM)
                .ok_or("--oven-folds past the pyramid's top")?;
            (
                TileSource::Oven(Server::open(worker_cnt, oven.clone(), min_zoom, limit)?),
                min_zoom,
            )
        }
    };
    let coverage: Vec<String> = Coverage::of_source()?.shards().map(|s| s.name()).collect();
    let app = Arc::new(App {
        tile_source,
        oven: oven.clone(),
        min_zoom,
        coverage,
        asset_dir,
        out,
    });
    // A declaration a client would refuse is refused here, at the start.
    decl(&app)?;

    let router = Router::new()
        .route(&format!("{TILE_ROOT}/source"), get(src))
        .route(&format!("{TILE_ROOT}/{{kind}}/{{*object}}"), get(tile))
        .route(&format!("{ASSET_ROOT}/{{*name}}"), get(asset))
        .with_state(app.clone());

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let served = runtime.block_on(async move {
        let listener = TcpListener::bind((host.as_str(), port))
            .await
            .map_err(|e| format!("{host}:{port}: {e}"))?;
        println!("[tile] listening on {host}:{port}");
        axum::serve(listener, router)
            .await
            .map_err(|e| e.to_string())
    });
    drop(runtime);
    drop(app);
    served
}

fn main() -> ExitCode {
    cli::main("tile", run)
}
