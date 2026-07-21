use super::*;
use std::fs;
use std::path::Path;

fn write_interface(net_root: &Path, name: &str, mac: &str, operstate: &str) {
    let iface = net_root.join(name);
    fs::create_dir_all(&iface).unwrap();
    fs::write(iface.join("address"), format!("{mac}\n")).unwrap();
    fs::write(iface.join("operstate"), format!("{operstate}\n")).unwrap();
}

#[test]
fn picks_the_only_physical_interface() {
    let dir = tempfile::tempdir().unwrap();
    write_interface(dir.path(), "lo", "00:00:00:00:00:00", "unknown");
    write_interface(dir.path(), "eth0", "aa:bb:cc:dd:ee:ff", "up");

    assert_eq!(detect_primary_mac(dir.path().to_str().unwrap()), Some("aa:bb:cc:dd:ee:ff".to_string()));
}

#[test]
fn skips_loopback_and_virtual_interfaces() {
    let dir = tempfile::tempdir().unwrap();
    write_interface(dir.path(), "lo", "00:00:00:00:00:00", "unknown");
    write_interface(dir.path(), "docker0", "02:42:ac:11:00:01", "up");
    write_interface(dir.path(), "veth1234", "1a:2b:3c:4d:5e:6f", "up");
    write_interface(dir.path(), "wlan0", "aa:bb:cc:dd:ee:11", "down");

    assert_eq!(detect_primary_mac(dir.path().to_str().unwrap()), Some("aa:bb:cc:dd:ee:11".to_string()));
}

#[test]
fn prefers_an_interface_that_is_up_over_one_that_is_down() {
    let dir = tempfile::tempdir().unwrap();
    write_interface(dir.path(), "eth0", "aa:aa:aa:aa:aa:aa", "down");
    write_interface(dir.path(), "wlan0", "bb:bb:bb:bb:bb:bb", "up");

    assert_eq!(detect_primary_mac(dir.path().to_str().unwrap()), Some("bb:bb:bb:bb:bb:bb".to_string()));
}

#[test]
fn returns_none_when_no_physical_interface_has_a_mac() {
    let dir = tempfile::tempdir().unwrap();
    write_interface(dir.path(), "lo", "00:00:00:00:00:00", "unknown");

    assert_eq!(detect_primary_mac(dir.path().to_str().unwrap()), None);
}

#[test]
fn returns_none_when_the_path_does_not_exist() {
    assert_eq!(detect_primary_mac("/nonexistent/path/for/testing"), None);
}

#[test]
fn shutdown_command_is_systemctl_poweroff() {
    // Pure/data only - never actually executed here (see the module doc
    // comment): running this for real would power off whatever machine
    // runs `cargo test`.
    assert_eq!(shutdown_command(), ("systemctl", ["poweroff"].as_slice()));
}
