// So' compilado em testes - processo fake com "AppId=<id>" no cmdline,
// compartilhado pelos testes de server.rs (is_app_id_running) e
// session.rs (watchdog de fechamento autonomo), pra testar a deteccao de
// processo contra o SO de verdade em vez de mockar sysinfo.
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
            .expect("falha ao spawnar processo fake pro teste");
        Self { child }
    }
}

impl Drop for FakeGameProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
