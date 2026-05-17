use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use canic_testkit::pic::acquire_pic_serial_guard;

const READY_FILE_ENV: &str = "DOMM_PIC_LOCK_PROBE_READY_FILE";
const HELPER_TEST: &str = "hold_pic_lock_for_parallel_probe";
const HELPER_HOLD_MS: u64 = 800;
const READY_TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn pic_serial_guard_default_namespace_allows_parallel_worker_processes() {
    let root = unique_probe_dir();
    fs::create_dir_all(&root)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", root.display()));
    let first_ready = root.join("first.ready");
    let second_ready = root.join("second.ready");

    let mut first = spawn_lock_holder(&first_ready);
    wait_for_ready_file(&first_ready, READY_TIMEOUT);

    let mut second = spawn_lock_holder(&second_ready);
    wait_for_ready_file(&second_ready, Duration::from_millis(400));

    assert!(
        first
            .try_wait()
            .expect("first helper status should be readable")
            .is_none(),
        "first helper should still hold its process-local lock when the second helper is ready"
    );

    assert_success(first.wait().expect("first helper should exit"), "first");
    assert_success(second.wait().expect("second helper should exit"), "second");
    let _ = fs::remove_dir_all(root);
}

#[test]
#[ignore = "helper spawned by pic_serial_guard_default_namespace_allows_parallel_worker_processes"]
fn hold_pic_lock_for_parallel_probe() {
    let ready_file = env::var_os(READY_FILE_ENV)
        .map(PathBuf::from)
        .expect("helper ready file env should be set");
    let _guard = acquire_pic_serial_guard();
    fs::write(&ready_file, b"ready\n")
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", ready_file.display()));
    thread::sleep(Duration::from_millis(HELPER_HOLD_MS));
}

fn spawn_lock_holder(ready_file: &Path) -> Child {
    Command::new(env::current_exe().expect("current test executable should resolve"))
        .args([
            "--ignored",
            "--exact",
            HELPER_TEST,
            "--test-threads=1",
            "--quiet",
        ])
        .env(READY_FILE_ENV, ready_file)
        .env_remove("CANIC_POCKET_IC_LOCK_NAMESPACE")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("lock holder helper should spawn")
}

fn wait_for_ready_file(path: &Path, timeout: Duration) {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }

    panic!(
        "timed out waiting for helper ready file at {}",
        path.display()
    );
}

fn assert_success(status: std::process::ExitStatus, label: &str) {
    assert!(status.success(), "{label} helper failed with {status}");
}

fn unique_probe_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after Unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "domm-pic-lock-probe-{}-{nanos}",
        std::process::id()
    ))
}
