// pxl-api/db.rs

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

pub type DbError = Box<dyn std::error::Error + Send + Sync>;

/// The application pool, proved reachable before the port opens. `select 1` and
/// not a feature probe: db-main is an ordinary Postgres with nothing in it yet,
/// and what this asks is whether the credentials and the network are right.
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

    pool.get().await?.query_one("select 1", &[]).await?;
    println!("[db] main: ok");

    Ok(pool)
}
