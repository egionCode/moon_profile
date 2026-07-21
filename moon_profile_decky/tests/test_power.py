"""
get_host_status/fetch_host_mac/shutdown_host talk to the MoonProfile
Runner (see moon_profile_runner/src-tauri/src/server.rs's /health,
/system/mac, /system/shutdown routes). wake_host doesn't talk to the
Runner at all - the host is assumed to be off, it sends a Wake-on-LAN
magic packet (build_magic_packet, moonprofile_core.py) directly as a UDP
broadcast.
"""

import socket
import urllib.error


async def _save_config(plugin_module, **overrides):
    config = {
        "host": "192.168.1.6",
        "username": "u",
        "password": "p",
        "runner_port": 47991,
        "mac_address": "",
        **overrides,
    }
    await plugin_module.Plugin().save_config(config)
    return config


class FakeRunnerClientOnline:
    def __init__(self, host, port):
        pass

    def health(self):
        return {"ok": True}


class FakeRunnerClientOffline:
    def __init__(self, host, port):
        pass

    def health(self):
        raise urllib.error.URLError("connection refused")

    def get_mac(self):
        raise urllib.error.URLError("connection refused")

    def shutdown(self):
        raise urllib.error.URLError("connection refused")


class TestGetHostStatus:
    async def test_unconfigured_when_no_host_is_set(self, plugin_module):
        result = await plugin_module.Plugin().get_host_status()

        assert result == "unconfigured"

    async def test_online_when_the_runner_answers_health(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientOnline)

        result = await plugin_module.Plugin().get_host_status()

        assert result == "online"

    async def test_offline_when_the_runner_is_unreachable(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientOffline)

        result = await plugin_module.Plugin().get_host_status()

        assert result == "offline"


class TestFetchHostMac:
    async def test_requires_a_configured_host(self, plugin_module):
        result = await plugin_module.Plugin().fetch_host_mac()

        assert result == {"ok": False, "error": "Configure the Apollo host first (Apollo Config tab)"}

    async def test_persists_the_detected_mac_into_the_config(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)

        class FakeRunnerClient:
            def __init__(self, host, port):
                pass

            def get_mac(self):
                return {"mac": "aa:bb:cc:dd:ee:ff"}

        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClient)

        result = await plugin_module.Plugin().fetch_host_mac()

        assert result == {"ok": True, "mac": "aa:bb:cc:dd:ee:ff"}
        assert (await plugin_module.Plugin().get_config())["mac_address"] == "aa:bb:cc:dd:ee:ff"

    async def test_reports_an_error_when_the_runner_could_not_detect_a_mac(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)

        class FakeRunnerClientNoMac:
            def __init__(self, host, port):
                pass

            def get_mac(self):
                return {"mac": None}

        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientNoMac)

        result = await plugin_module.Plugin().fetch_host_mac()

        assert result["ok"] is False

    async def test_reports_an_error_when_the_runner_is_unreachable(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientOffline)

        result = await plugin_module.Plugin().fetch_host_mac()

        assert result["ok"] is False
        assert "Runner" in result["error"]


class TestShutdownHost:
    async def test_requires_a_configured_host(self, plugin_module):
        result = await plugin_module.Plugin().shutdown_host()

        assert result == {"ok": False, "error": "Configure the Apollo host first (Apollo Config tab)"}

    async def test_returns_the_runners_response_when_reachable(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)

        class FakeRunnerClient:
            def __init__(self, host, port):
                pass

            def shutdown(self):
                return {"ok": True}

        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClient)

        result = await plugin_module.Plugin().shutdown_host()

        assert result == {"ok": True}

    async def test_reports_an_error_when_the_runner_is_unreachable(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)

        class FakeRunnerClientUnreachable:
            def __init__(self, host, port):
                pass

            def shutdown(self):
                raise urllib.error.URLError("connection refused")

        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientUnreachable)

        result = await plugin_module.Plugin().shutdown_host()

        assert result["ok"] is False
        assert "Runner" in result["error"]


class TestWakeHost:
    async def test_requires_a_saved_mac_address(self, plugin_module):
        await _save_config(plugin_module)

        result = await plugin_module.Plugin().wake_host()

        assert result["ok"] is False

    async def test_rejects_a_malformed_saved_mac_address(self, plugin_module):
        await _save_config(plugin_module, mac_address="not-a-mac")

        result = await plugin_module.Plugin().wake_host()

        assert result["ok"] is False

    async def test_sends_a_broadcast_udp_packet_to_port_9(self, plugin_module, monkeypatch):
        # Real socket (bound to a random port, not port 9 - that needs
        # root), same spirit as the rest of the project: confirms the
        # real sendto() call instead of just mocking socket.socket.
        await _save_config(plugin_module, mac_address="aa:bb:cc:dd:ee:ff")

        received = {}
        real_socket = socket.socket

        class RecordingSocket:
            def __init__(self, *args, **kwargs):
                self._sock = real_socket(*args, **kwargs)

            def setsockopt(self, *args, **kwargs):
                self._sock.setsockopt(*args, **kwargs)

            def sendto(self, data, addr):
                received["data"] = data
                received["addr"] = addr

            def __enter__(self):
                return self

            def __exit__(self, *exc):
                self._sock.close()

        monkeypatch.setattr(socket, "socket", RecordingSocket)

        result = await plugin_module.Plugin().wake_host()

        assert result == {"ok": True}
        assert received["addr"] == ("255.255.255.255", 9)
        assert received["data"][:6] == b"\xff" * 6
