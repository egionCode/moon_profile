// Only compiled in tests - fake process with "AppId=<id>" in its
// cmdline, shared by the server.rs tests (is_app_id_running) and
// session.rs (autonomous-close watchdog), to test process detection
// against the real OS instead of mocking sysinfo.
use std::process::Child;

pub(crate) struct FakeGameProcess {
    child: Child,
}

impl FakeGameProcess {
    pub(crate) fn spawn(app_id: &str) -> Self {
        let marker = format!("AppId={app_id}");
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("exec -a \"{marker}\" sleep 30"))
            .spawn()
            .expect("failed to spawn fake process for the test");
        Self { child }
    }
}

impl Drop for FakeGameProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
