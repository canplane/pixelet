// pxl-geo-bake/oven/cache.test.rs

use super::*;
use crate::util;
use pxl_spec::directory;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time;

/// The folder a test's caches lie in, under the checkout's `tmp/`, removed when the test ends.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A cache in a folder of the test's own, with the folder; what the test makes beside the
/// cache lies in the folder too.
fn cache() -> (Arc<Cache>, Folder) {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    let folder = Folder(directory::tmp_dir(&format!(
        "oven-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )));
    let cache = Cache::new(
        folder.0.join("cache"),
        Arc::new(Semaphore::new(32)),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    (Arc::new(cache), folder)
}

#[tokio::test]
async fn concurrent_publish_once() {
    let (cache, _folder) = cache();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tasks = Vec::new();
    for _ in 0..10 {
        let c = cache.clone();
        let calls = calls.clone();
        tasks.push(tokio::spawn(async move {
            let producer = c.clone();
            c.once("tile".into(), true, move |_deadline| async move {
                calls.fetch_add(1, Ordering::Relaxed);
                producer
                    .publish("tile", Some(b"packet"))
                    .map_err(Failure::from)
            })
            .await
        }));
    }
    for t in tasks {
        t.await.unwrap().unwrap();
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        util::unpack(&cache.read("tile").await.unwrap().unwrap()).unwrap(),
        b"packet"
    );
    cache.publish("empty", None).unwrap();
    assert_eq!(cache.read("empty").await.unwrap(), None);
    assert!(cache.read("missing").await.is_err());
    assert!(cache
        .once("broken".into(), true, |_deadline| async {
            Err(Failure::Bake("source failed".into()))
        })
        .await
        .is_err());
    assert!(cache
        .once("broken".into(), true, |_deadline| async { Ok(()) })
        .await
        .is_err());
}

#[tokio::test]
async fn disconnect_keeps_publication() {
    let (c, _folder) = cache();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let task = {
        let c = c.clone();
        let entered = entered.clone();
        let release = release.clone();
        tokio::spawn(async move {
            let producer = c.clone();
            c.once("tile".into(), true, move |_deadline| async move {
                entered.notify_one();
                release.notified().await;
                producer
                    .publish("tile", Some(b"tile"))
                    .map_err(Failure::from)
            })
            .await
        })
    };
    entered.notified().await;
    task.abort();
    release.notify_one();
    c.once("tile".into(), true, |_deadline| async {
        panic!("must join existing job")
    })
    .await
    .unwrap();
    assert_eq!(
        util::unpack(&c.read("tile").await.unwrap().unwrap()).unwrap(),
        b"tile"
    );
}

#[tokio::test]
async fn shared_admission() {
    let (a, _folder) = cache();
    let b = Cache::new(
        a.root.with_extension("other"),
        a.admission.clone(),
        a.deadline,
    )
    .unwrap();
    let producer = a.clone();
    a.once("cached".into(), true, move |_deadline| async move {
        producer
            .publish("cached", Some(b"packet"))
            .map_err(Failure::from)
    })
    .await
    .unwrap();
    let permits = a.admission.clone().acquire_many_owned(32).await.unwrap();
    a.once("cached".into(), true, |_deadline| async {
        panic!("must be cached")
    })
    .await
    .unwrap();
    assert!(matches!(
        b.once("new".into(), true, |_deadline| async {
            panic!("must not start")
        })
        .await,
        Err(Failure::Limited(_))
    ));
    drop(permits);
    b.once("new".into(), true, |_deadline| async { Ok(()) })
        .await
        .unwrap();
}

#[tokio::test]
async fn expired_work_joinable() {
    let (made, _folder) = cache();
    let c = Arc::new(
        Cache::new(
            made.root.with_extension("cleanup"),
            Arc::new(Semaphore::new(1)),
            std::time::Duration::from_millis(20),
        )
        .unwrap(),
    );
    let cleaning = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let first = {
        let c = c.clone();
        let cleaning = cleaning.clone();
        let release = release.clone();
        tokio::spawn(async move {
            c.once("tile".into(), true, move |lease| async move {
                time::sleep_until(lease.deadline).await;
                cleaning.notify_one();
                release.notified().await;
                Err(Failure::Limited("deadline"))
            })
            .await
        })
    };
    cleaning.notified().await;
    assert!(!first.is_finished());
    assert_eq!(c.admission.available_permits(), 0);
    let joined = c.once("tile".into(), true, |_| async {
        panic!("expired work must still own this key")
    });
    tokio::pin!(joined);
    tokio::select! {
        biased;
        _ = &mut joined => panic!("join must wait for cleanup"),
        _ = std::future::ready(()) => {}
    }
    release.notify_one();
    assert!(matches!(first.await.unwrap(), Err(Failure::Limited(_))));
    assert!(matches!(joined.await, Err(Failure::Limited(_))));
    assert_eq!(c.admission.available_permits(), 1);
    c.once("tile".into(), true, |_| async { Ok(()) })
        .await
        .unwrap();
}
