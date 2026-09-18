// pxl-geo/db.rs

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

/// Anything that went wrong reaching or reading the database — checking a
/// connection out of the pool and running a statement fail in different types
/// and are the same thing to every caller here, which either answers or 500s.
pub type DbError = Box<dyn std::error::Error + Send + Sync>;

/// The geo pool, proved reachable before the port opens. The probe is
/// `postgis_full_version()` rather than `select 1` because a plain Postgres
/// answering on the geo URL is a misconfiguration this would otherwise only
/// surface on the first query that needs a geometry.
pub async fn connect(url: &str, pool_size: usize) -> Result<Pool, DbError> {
    let pg_cfg: tokio_postgres::Config = url.parse()?;
    let mgr = Manager::from_config(
        pg_cfg,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    let pool = Pool::builder(mgr).max_size(pool_size).build()?;

    pool.get().await?.query_one("select postgis_full_version()", &[]).await?;
    println!("[db] geo: ok");

    Ok(pool)
}
