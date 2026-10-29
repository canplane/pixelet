// pxl-server/db.rs

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

pub type DbError = Box<dyn std::error::Error + Send + Sync>;

/// Run once before the port opens, to prove the pool reached the right
/// database: pxl-geo asks `postgis_full_version()` so a plain Postgres fails at boot.
pub struct Probe<'a> {
    pub sql: &'a str,
    pub name: &'a str,
}

pub async fn connect(url: &str, pool_sz: usize, probe: &Probe<'_>) -> Result<Pool, DbError> {
    let pg_cfg: tokio_postgres::Config = url.parse()?;
    let mgr = Manager::from_config(
        pg_cfg,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    let pool = Pool::builder(mgr).max_size(pool_sz).build()?;

    pool.get().await?.query_one(probe.sql, &[]).await?;
    println!("[db] {}: ok", probe.name);

    Ok(pool)
}
