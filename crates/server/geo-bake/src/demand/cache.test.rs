// pxl-geo-bake/demand/cache.test.rs

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn cache() -> Arc<Cache> {
    static SERIAL: AtomicUsize = AtomicUsize::new(0);
    Arc::new(
        Cache::new(
            std::env::temp_dir().join(format!("pxl-demand-{}-{}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed))),
            Arc::new(Semaphore::new(32)),
            std::time::Duration::from_secs(30),
        )
        .unwrap(),
    )
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
            c.once("tile".into(), true, move |_deadline| async move {
                calls.fetch_add(1, Ordering::Relaxed);
                producer.publish("tile", Some(b"packet")).map_err(Failure::from)
            })
            .await
        }));
    }
    for t in tasks {
        t.await.unwrap().unwrap();
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(crate::util::unpack(&cache.read("tile").await.unwrap().unwrap()).unwrap(), b"packet");
    cache.publish("empty", None).unwrap();
    assert_eq!(cache.read("empty").await.unwrap(), None);
    assert!(cache.read("missing").await.is_err());
    assert!(cache.once("broken".into(), true, |_deadline| async { Err(Failure::Bake("source failed".into())) }).await.is_err());
    assert!(cache.once("broken".into(), true, |_deadline| async { Ok(()) }).await.is_err());
}

#[tokio::test]
async fn disconnected_request_does_not_cancel_publication() {
    let c = cache();
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
                producer.publish("tile", Some(b"tile")).map_err(Failure::from)
            })
            .await
        })
    };
    entered.notified().await;
    task.abort();
    release.notify_one();
    c.once("tile".into(), true, |_deadline| async { panic!("must join existing job") }).await.unwrap();
    assert_eq!(crate::util::unpack(&c.read("tile").await.unwrap().unwrap()).unwrap(), b"tile");
}

#[tokio::test]
async fn kinds_share_admission_but_hits_and_joiners_need_no_new_slot() {
    let a = cache();
    let b = Cache::new(a.root.with_extension("other"), a.admission.clone(), a.deadline).unwrap();
    let producer = a.clone();
    a.once(
        "cached".into(),
        true,
        move |_deadline| async move { producer.publish("cached", Some(b"packet")).map_err(Failure::from) },
    )
    .await
    .unwrap();
    let held = a.admission.clone().acquire_many_owned(32).await.unwrap();
    a.once("cached".into(), true, |_deadline| async { panic!("must be cached") }).await.unwrap();
    assert!(matches!(b.once("new".into(), true, |_deadline| async { panic!("must not start") }).await, Err(Failure::Limited(_))));
    drop(held);
    b.once("new".into(), true, |_deadline| async { Ok(()) }).await.unwrap();
}

#[tokio::test]
async fn expired_work_stays_joinable_and_admitted_until_its_cleanup_finishes() {
    let c = Arc::new(Cache::new(cache().root.with_extension("cleanup"), Arc::new(Semaphore::new(1)), std::time::Duration::from_millis(20)).unwrap());
    let cleaning = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let first = {
        let c = c.clone();
        let cleaning = cleaning.clone();
        let release = release.clone();
        tokio::spawn(async move {
            c.once("tile".into(), true, move |lease| async move {
                tokio::time::sleep_until(lease.deadline).await;
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
    let joined = c.once("tile".into(), true, |_| async { panic!("expired work must still own this key") });
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
    c.once("tile".into(), true, |_| async { Ok(()) }).await.unwrap();
}

#[tokio::test]
async fn a_published_key_answers_from_the_disk_and_holds_no_flight() {
    let c = cache();
    let producer = c.clone();
    c.once("tile".into(), true, move |_deadline| async move { producer.publish("tile", None).map_err(Failure::from) })
        .await
        .unwrap();
    assert!(c.flights.lock().unwrap().is_empty(), "a published result is the disk's, not the map's");
    c.once("tile".into(), true, |_deadline| async { panic!("published work must not run again") }).await.unwrap();
    assert!(c.once("unpublished".into(), true, |_deadline| async { Err(Failure::Bake("source failed".into())) }).await.is_err());
    assert_eq!(c.flights.lock().unwrap().len(), 1, "a source failure is remembered for the generation");
}
