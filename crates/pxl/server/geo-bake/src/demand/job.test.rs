// pxl-geo-bake/demand/job.test.rs

use super::*;
use std::time::Duration;
use tokio::time::Instant;

#[tokio::test]
async fn a_generation_deadline_kills_and_reaps_the_process() {
    let path = std::env::temp_dir().join(format!("pxl-demand-deadline-{}", std::process::id()));
    let mut command = Command::new("sh");
    command.arg("-c").arg("echo $$ > \"$1\"; exec sleep 30").arg("deadline-test").arg(&path).kill_on_drop(true);
    assert!(matches!(supervise(command, Instant::now() + Duration::from_millis(200)).await, Err(Failure::Limited(_))));
    let pid = std::fs::read_to_string(&path).unwrap();
    let status = std::process::Command::new("kill").args(["-0", pid.trim()]).stderr(std::process::Stdio::null()).status().unwrap();
    assert!(!status.success(), "deadline must release the process, not just the waiter");
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn success_and_failure_are_not_reported_as_empty_or_timeout() {
    let mut ok = Command::new("sh");
    ok.args(["-c", "exit 0"]);
    supervise(ok, Instant::now() + Duration::from_secs(2)).await.unwrap();
    let mut bad = Command::new("sh");
    bad.args(["-c", "exit 1"]);
    assert!(matches!(supervise(bad, Instant::now() + Duration::from_secs(2)).await, Err(Failure::Bake(_))));
}

#[test]
fn complete_artifacts_are_moved_from_staging_and_abandoned_work_is_not_published() {
    let base = std::env::temp_dir().join(format!("pxl-demand-publication-{}", std::process::id()));
    let runner = Runner::new(String::new(), 1, Limits::default()).unwrap();
    let data = Dataset::open(&base, super::super::Kind::Poi, &runner).unwrap();
    let stage = Staging(data.cache.root.join("stage"));
    std::fs::create_dir(&stage.0).unwrap();
    write_packet(&stage.0, Some(b"packet")).unwrap();
    stage.publish(&data, Publication::Packet("tile".into())).unwrap();
    assert_eq!(std::fs::read(data.cache.root.join("tile")).unwrap(), b"packet");
    assert!(!stage.0.join("packet").exists());
    let dictionary = stage.0.join("dictionary");
    std::fs::create_dir(&dictionary).unwrap();
    std::fs::write(dictionary.join("backing"), b"ids").unwrap();
    write_packet(&dictionary, Some(b"names")).unwrap();
    assert!(!data.cache.root.join("dict").exists());
    stage
        .publish(
            &data,
            Publication::Directory {
                source: "dictionary".into(),
                target: "dict".into(),
            },
        )
        .unwrap();
    assert_eq!(std::fs::read(data.cache.root.join("dict/backing")).unwrap(), b"ids");
    assert_eq!(std::fs::read(data.cache.root.join("dict/packet")).unwrap(), b"names");
    write_packet(&stage.0, Some(b"incomplete")).unwrap();
    drop(stage);
    assert!(!data.cache.root.join("stage").exists());
    assert!(!data.cache.root.join("packet").exists());
}

#[tokio::test]
async fn waiting_does_not_renew_the_execution_budget() {
    let command = Command::new("/nonexistent/should-not-start");
    let expired = Instant::now() - Duration::from_secs(1);
    assert!(matches!(supervise(command, expired).await, Err(Failure::Limited(_))));
}
