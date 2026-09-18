// pxl-server/db.rs

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

/// Anything that went wrong reaching or reading the database — checking a
/// connection out of the pool and running a statement fail in different types
/// and are the same thing to every caller here, which either answers or 500s.
pub type DbError = Box<dyn std::error::Error + Send + Sync>;

/// What proves the pool reached the *right* database, run once before the
/// port opens.
///
/// pxl-geo asks `postgis_full_version()` rather than `select 1`, because a
/// plain Postgres answering on the geo URL is a misconfiguration that would
/// otherwise surface on the first query needing a geometry. pxl-api has
/// nothing to probe for — db-main is an ordinary Postgres with nothing in it
/// yet — so `select 1` asks what it can: are the credentials and the network
/// right.
pub struct Probe<'a> {
    pub sql: &'a str,
    /// What the readiness line calls this database.
    pub name: &'a str,
}

/// The pool, proved reachable before the port opens.
pub async fn connect(url: &str, pool_size: usize, probe: &Probe<'_>) -> Result<Pool, DbError> {
    let pg_cfg: tokio_postgres::Config = url.parse()?;
    let mgr = Manager::from_config(
        pg_cfg,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    let pool = Pool::builder(mgr).max_size(pool_size).build()?;

    pool.get().await?.query_one(probe.sql, &[]).await?;
    println!("[db] {}: ok", probe.name);

    Ok(pool)
}
