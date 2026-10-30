// pxl-geo-bake/demand/job.test.rs

use super::*;
use pxl_spec::dirs;
use std::time::Duration;
use tokio::task;
use tokio::time::Instant;

#[tokio::test]
async fn outcomes_reported() {
    let mut ok = Command::new("sh");
    ok.args(["-c", "exit 0"]);
    supervise(ok, Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let mut bad = Command::new("sh");
    bad.args(["-c", "exit 1"]);
    assert!(matches!(
        supervise(bad, Instant::now() + Duration::from_secs(2)).await,
        Err(Failure::Bake(_))
    ));
}

#[test]
fn staging_publish() {
    let base = dirs::temp_dir(&format!("demand-publication-{}", std::process::id()));
    let runner = Runner::new(1, Limits::default()).unwrap();
    let data = Dataset::open(&base, super::super::Kind::Place, &runner).unwrap();
    let stage = Staging(data.cache.root.join("stage"));
    std::fs::create_dir(&stage.0).unwrap();
    write_pkt(&stage.0, Some(b"packet")).unwrap();
    stage.publish(&data, "tile").unwrap();
    assert_eq!(
        std::fs::read(data.cache.root.join("tile")).unwrap(),
        b"packet"
    );
    assert!(!stage.0.join("packet").exists());
    write_pkt(&stage.0, Some(b"incomplete")).unwrap();
    drop(stage);
    assert!(!data.cache.root.join("stage").exists());
    assert!(!data.cache.root.join("packet").exists());
}

#[tokio::test]
async fn wait_keeps_budget() {
    let command = Command::new("/nonexistent/should-not-start");
    let expired = Instant::now() - Duration::from_secs(1);
    assert!(matches!(
        supervise(command, expired).await,
        Err(Failure::Limited(_))
    ));
}

#[tokio::test]
async fn client_wait_first() {
    let workers = Arc::new(Workers {
        state: Mutex::new((1, VecDeque::new())),
    });
    let lease = |asked: bool| Lease {
        deadline: Instant::now() + Duration::from_secs(5),
        asked: Arc::new(AtomicBool::new(asked)),
    };
    let held = workers.acquire(&lease(true)).await.unwrap();
    let order = Arc::new(Mutex::new(Vec::new()));
    let wait = |name: &'static str, l: Lease| {
        let (workers, order) = (workers.clone(), order.clone());
        tokio::spawn(async move {
            let _worker = workers.acquire(&l).await.unwrap();
            order.lock().unwrap().push(name);
        })
    };
    let fold = wait("fold", lease(false));
    task::yield_now().await;
    let asked = wait("asked", lease(true));
    task::yield_now().await;
    drop(held);
    asked.await.unwrap();
    fold.await.unwrap();
    assert_eq!(*order.lock().unwrap(), ["asked", "fold"]);
}
