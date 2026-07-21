use super::*;

// Real fixture, captured by actually running "ping -c 1 -W 1 <ip>" on
// the device (trimmed to the relevant lines).
const REAL_REPLY: &str = "PING 192.168.1.50 (192.168.1.50) 56(84) bytes of data.\n64 bytes from 192.168.1.50: icmp_seq=1 ttl=64 time=3.21 ms\n\n--- 192.168.1.50 ping statistics ---\n1 packets transmitted, 1 received, 0% packet loss, time 0ms\nrtt min/avg/max/mdev = 3.210/3.210/3.210/0.000 ms\n";

#[test]
fn parses_the_round_trip_time_from_a_real_reply() {
    assert_eq!(parse_ping_output(REAL_REPLY), Some(3.21));
}

#[test]
fn returns_none_when_there_is_no_time_field_at_all() {
    let no_reply = "PING 10.255.255.1 (10.255.255.1) 56(84) bytes of data.\n\n--- 10.255.255.1 ping statistics ---\n1 packets transmitted, 0 received, 100% packet loss, time 0ms\n";

    assert_eq!(parse_ping_output(no_reply), None);
}

#[test]
fn returns_none_on_garbage_output() {
    assert_eq!(parse_ping_output("not ping output at all"), None);
}

#[test]
fn returns_none_when_the_ping_binary_does_not_exist() {
    // Doesn't ping anything real over the network - just confirms the
    // fail-open path when the command itself can't even be spawned.
    assert_eq!(ping_once_with("/nonexistent/ping-binary-xyz", "127.0.0.1"), None);
}
