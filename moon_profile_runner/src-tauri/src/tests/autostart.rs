use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

// Fake `systemctl` (a plain shell script) instead of mocking - same
// spirit as the rest of the project's process-based tests (see
// session.rs). Logs every subcommand it's invoked with (arg[1], e.g.
// "is-enabled" or "enable") into a marker file, so the test can assert
// on the exact call sequence.
//
// Writes the script via a plain tempdir instead of `NamedTempFile`:
// `NamedTempFile` keeps its own file descriptor open for writing, and
// executing a file that still has an open write handle fails with
// ETXTBSY ("Text file busy") on Linux - a real failure hit while
// writing this test, not a hypothetical one.
fn fake_systemctl(is_enabled_exit_code: u8) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let marker_path = dir.path().join("calls.log");
    let script_path = dir.path().join("systemctl");

    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\necho \"$2\" >> {}\nif [ \"$2\" = \"is-enabled\" ]; then exit {is_enabled_exit_code}; fi\nexit 0\n",
            marker_path.display()
        ),
    )
    .unwrap();
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    (dir, marker_path)
}

#[test]
fn does_not_call_enable_when_already_enabled() {
    let (dir, marker_path) = fake_systemctl(0); // 0 = "is-enabled" reports success
    let script_path = dir.path().join("systemctl");

    ensure_enabled_with(script_path.to_str().unwrap(), "moon-profile-runner.service");

    let calls = fs::read_to_string(&marker_path).unwrap();
    assert_eq!(calls.lines().collect::<Vec<_>>(), vec!["is-enabled"]);
}

#[test]
fn calls_enable_when_not_yet_enabled() {
    let (dir, marker_path) = fake_systemctl(1); // non-zero = not enabled yet
    let script_path = dir.path().join("systemctl");

    ensure_enabled_with(script_path.to_str().unwrap(), "moon-profile-runner.service");

    let calls = fs::read_to_string(&marker_path).unwrap();
    assert_eq!(calls.lines().collect::<Vec<_>>(), vec!["is-enabled", "enable"]);
}
