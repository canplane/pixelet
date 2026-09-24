// pxl-geo-bake/demand/cache.test.rs

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn cache() -> Arc<Cache> {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    Arc::new(Cache::new(std::env::temp_dir().join(format!("pxl-demand-{}-{}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed)))).unwrap())
}

#[tokio::test]
async fn concurrent_requests_publish_once_and_preserve_empty_and_failed_results() {
    let cache = cache();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..10 {
        let c = cache.clone();
        let calls = calls.clone();
        tasks.push(tokio::spawn(async move {
            let producer = c.clone();
            c.once("page".into(), async move {
                calls.fetch_add(1, Ordering::Relaxed);
                producer.publish("page", Some(b"packet"))
            })
            .await
        }));
    }
    for t in tasks {
        t.await.unwrap().unwrap();
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(crate::util::unpack(&cache.read("page").await.unwrap().unwrap()).unwrap(), b"packet");
    cache.publish("empty", None).unwrap();
    assert_eq!(cache.read("empty").await.unwrap(), None);
    assert!(cache.read("missing").await.is_err());
    assert!(cache.once("broken".into(), async { Err("source failed".into()) }).await.is_err());
    assert!(cache.once("broken".into(), async { Ok(()) }).await.is_err());
}

#[tokio::test]
async fn disconnected_request_does_not_cancel_publication_and_old_generation_cannot_write_into_new() {
    let old = cache();
    let new = cache();
    let path = old.root.clone();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let task = {
        let c = old.clone();
        let entered = entered.clone();
        let release = release.clone();
        tokio::spawn(async move {
            let producer = c.clone();
            c.once("page".into(), async move {
                entered.notify_one();
                release.notified().await;
                producer.publish("page", Some(b"old"))
            })
            .await
        })
    };
    entered.notified().await;
    task.abort();
    new.publish("page", Some(b"new")).unwrap();
    release.notify_one();
    old.once("page".into(), async { panic!("must join existing job") }).await.unwrap();
    assert_eq!(crate::util::unpack(&new.read("page").await.unwrap().unwrap()).unwrap(), b"new");
    drop(old);
    assert!(!path.exists());
}
