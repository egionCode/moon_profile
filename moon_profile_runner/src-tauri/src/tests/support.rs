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

    // No "AppId=<id>" anywhere in its own cmdline (only the compat-data
    // env var) - simulates the real Proton case that broke
    // is_app_id_running/kill_game_process on-device: the actual game exe
    // matches only via env_var_matches_app_id, not cmdline.
    pub(crate) fn spawn_with_compatdata_env(app_id: &str) -> Self {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .env("STEAM_COMPAT_DATA_PATH", format!("/home/deck/.steam/steam/steamapps/compatdata/{app_id}"))
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
