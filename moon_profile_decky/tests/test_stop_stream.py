"""
stop_stream() only talks to the MoonProfile Runner: it already knows which
app_id/credentials to use (registered by runner.py at launch, see
session.rs), kills the game if it's still alive, and restores the display
BEFORE notifying Apollo. The Runner is NOT optional anymore: Apollo has no
prep-cmd at all (neither do nor undo), so there's no sensible fallback to
call it directly, an error here is reported as a real error.
"""

import http.server
import json
import threading
import urllib.error


async def _save_config(plugin_module, **overrides):
    config = {
        "host": "192.168.1.6",
        "username": "u",
        "password": "p",
        "runner_port": 47991,
        **overrides,
    }
    await plugin_module.Plugin().save_config(config)
    return config


class FakeRunnerClientOk:
    def __init__(self, host, port):
        pass

    def close_session(self):
        return {"ok": True}


class FakeRunnerClientNoSession:
    def __init__(self, host, port):
        pass

    def close_session(self):
        return {"ok": False, "error": "No session registered with the Runner"}


class FakeRunnerClientUnreachable:
    def __init__(self, host, port):
        pass

    def close_session(self):
        raise urllib.error.URLError("connection refused")


class TestStopStreamUsesTheRunner:
    async def test_returns_ok_when_the_runner_closes_it(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientOk)

        result = await plugin_module.Plugin().stop_stream()

        assert result == {"ok": True}

    async def test_reports_the_runner_error_when_there_is_no_active_session(self, plugin_module, monkeypatch):
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientNoSession)

        result = await plugin_module.Plugin().stop_stream()

        assert result == {"ok": False, "error": "No session registered with the Runner"}

    async def test_reports_an_error_when_the_runner_is_unreachable(self, plugin_module, monkeypatch):
        # No fallback to calling Apollo directly: it has no prep-cmd at
        # all, calling it without the Runner would just drop the
        # connection without restoring the display or killing the game.
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientUnreachable)

        result = await plugin_module.Plugin().stop_stream()

        assert result["ok"] is False
        assert "Runner" in result["error"]

    async def test_requires_a_configured_host_before_trying_anything(self, plugin_module):
        result = await plugin_module.Plugin().stop_stream()

        assert result == {"ok": False, "error": "Configure the Apollo host first"}


class TestRunnerClientCloseSession:
    async def test_sends_a_post_to_session_close_and_parses_the_json_response(self, plugin_module):
        # Real HTTP server (not mocked) running on a free local port:
        # confirms the client's real behavior (method, path, response
        # parsing), same spirit as the rest of the project (test against
        # the real OS/network when feasible instead of just mocking).
        received = {}

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                received["method"] = self.command
                received["path"] = self.path
                body = json.dumps({"ok": True, "error": None}).encode()
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
            result = client.close_session()
        finally:
            thread.join(timeout=5)
            server.server_close()

        assert result == {"ok": True, "error": None}
        assert received["method"] == "POST"
        assert received["path"] == "/session/close"
