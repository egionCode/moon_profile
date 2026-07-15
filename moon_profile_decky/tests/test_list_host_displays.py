"""
list_host_displays() (main.py) e RunnerClient.list_displays() - alimentam
o ProfileEditor.tsx com os monitores de verdade do host (via
GET /displays no MoonProfile Runner, ver
moon_profile_runner/src-tauri/src/displays.rs).
"""

import http.server
import json
import threading


async def _save_config(plugin_module, **overrides):
    config = {
        "host": "127.0.0.1",
        "username": "u",
        "password": "p",
        "runner_port": 47991,
        **overrides,
    }
    await plugin_module.Plugin().save_config(config)
    return config


class TestRunnerClientListDisplays:
    async def test_sends_a_get_to_displays_and_parses_the_json_response(self, plugin_module):
        # Servidor HTTP real (nao mockado), mesmo espirito do resto do
        # projeto - confirma o comportamento de verdade do cliente.
        received = {}
        fake_displays = [
            {"name": "HDMI-A-1", "connected": True, "enabled": False},
            {"name": "DP-3", "connected": True, "enabled": True},
        ]

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                received["path"] = self.path
                body = json.dumps(fake_displays).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        port = server.server_address[1]
        thread = threading.Thread(target=server.handle_request)
        thread.start()
        try:
            client = plugin_module.RunnerClient("127.0.0.1", port)
            result = client.list_displays()
        finally:
            thread.join(timeout=5)
            server.server_close()

        assert received["path"] == "/displays"
        assert result == fake_displays


class TestListHostDisplays:
    async def test_requires_a_configured_host(self, plugin_module):
        result = await plugin_module.Plugin().list_host_displays()

        assert result == {
            "ok": False,
            "error": "Configure o host do Apollo primeiro (aba Config do Apollo)",
            "displays": [],
        }

    async def test_returns_the_runners_display_list_when_reachable(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)

        class FakeRunnerClient:
            def __init__(self, host, port):
                pass

            def list_displays(self):
                return [{"name": "HDMI-A-1", "connected": True, "enabled": False}]

        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClient)

        result = await plugin_module.Plugin().list_host_displays()

        assert result == {"ok": True, "displays": [{"name": "HDMI-A-1", "connected": True, "enabled": False}]}

    async def test_reports_an_error_when_the_runner_is_unreachable(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)

        class FakeRunnerClientUnreachable:
            def __init__(self, host, port):
                pass

            def list_displays(self):
                import urllib.error

                raise urllib.error.URLError("connection refused")

        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientUnreachable)

        result = await plugin_module.Plugin().list_host_displays()

        assert result["ok"] is False
        assert result["displays"] == []
