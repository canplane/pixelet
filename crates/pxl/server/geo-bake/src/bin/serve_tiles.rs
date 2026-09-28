// pxl-geo-bake/bin/serve_tiles.rs

//! Local demand bakes. Each tile kind uses the same generation, job and
//! response path; the deployment still serves immutable bucket objects.
//! --cache is the parent of private terrain/poi/geocode generation directories.

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
use pxl_geo_bake::util::CONTENT_ENCODING;
use pxl_spec::route::{ASSET_ROOT, GEO_INTERNAL, TILE_ROOT};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

struct App {
    demand: Server,
    /// The coarsest level served whole, `--top-level`: what the client's floor
    /// is leased at.
    top_level: u32,
    asset_dir: PathBuf,
    admin_token: Option<String>,
}

async fn tile(State(app): State<Arc<App>>, Path((kind, name)): Path<(String, String)>) -> Response {
    let Some(kind) = Kind::parse(&kind) else { return StatusCode::NOT_FOUND.into_response() };
    match app.demand.get(kind, &name).await {
        Ok(Some(bytes)) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CONTENT_ENCODING, CONTENT_ENCODING),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Bytes::from(bytes),
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(Failure::BadName) => (StatusCode::BAD_REQUEST, "bad tile name").into_response(),
        Err(Failure::Unavailable) => (StatusCode::SERVICE_UNAVAILABLE, "past --top-level").into_response(),
        Err(Failure::Limited(reason)) => (StatusCode::SERVICE_UNAVAILABLE, reason).into_response(),
        Err(Failure::Bake(e)) => {
            eprintln!("[tile/{name}] {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// What this source declares of itself, in the form a published bake's
/// `SOURCE` has (spec/pxl/spec/bake/published.json), so the client reads
/// either the same way and learns its floor from the source rather than from
/// what fails (pxl-kernel world/paging/floor.rs).
async fn source(State(app): State<Arc<App>>) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json"), (header::CACHE_CONTROL, "no-store")],
        serde_json::json!({ "top_level": app.top_level }).to_string(),
    )
        .into_response()
}

async fn invalidate(State(app): State<Arc<App>>, headers: HeaderMap, Path(kind): Path<String>, body: String) -> Response {
    if let Some(token) = &app.admin_token {
        if headers.get("x-admin-token").and_then(|v| v.to_str().ok()) != Some(token.as_str()) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }
    let Some(kind) = Kind::parse(&kind) else { return StatusCode::NOT_FOUND.into_response() };
    if !body.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "invalidate the whole dataset: send no body").into_response();
    }
    match app.demand.invalidate(kind) {
        Ok(()) => (StatusCode::OK, "invalidated all").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
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
    let mut cache = PathBuf::from("./out/dev");
    let mut top_level: u32 = 3;

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
            "--top-level" => top_level = flags.parse()?,
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let app = Arc::new(App {
        demand: Server::open(&url, workers, cache, top_level, limits)?,
        top_level,
        asset_dir,
        admin_token: std::env::var("ADMIN_TOKEN").ok().filter(|t| !t.is_empty()),
    });

    // The roots and branches are spec/pxl/spec/route.json's, as the client's and the
    // relays' are; what each one carries past them is this server's.
    let router = Router::new()
        .route(&format!("{TILE_ROOT}/source"), get(source))
        .route(&format!("{TILE_ROOT}/{{kind}}/{{name}}"), get(tile))
        // Its own root: an asset is neither server's, and its URL is its bucket
        // key with a leading slash.
        .route(&format!("{ASSET_ROOT}/{{*name}}"), get(asset))
        .route(&format!("{GEO_INTERNAL}/world/{{kind}}/cache/invalidate"), post(invalidate))
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
