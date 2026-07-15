"""
stop_stream() so' fala com o MoonProfile Runner - ele ja sabe qual
app_id/credenciais usar (registrados por runner.py no lancamento, ver
session.rs), mata o jogo se ainda estiver vivo e restaura a tela ANTES de
avisar o Apollo. O Runner NAO e' mais opcional: o Apollo nao tem prep-cmd
nenhum (nem do, nem undo), entao nao ha' fallback sensato pra chamar ele
direto - um erro aqui e' reportado como erro de verdade.
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
        return {"ok": False, "error": "Nenhuma sessao registrada no Runner"}


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

        assert result == {"ok": False, "error": "Nenhuma sessao registrada no Runner"}

    async def test_reports_an_error_when_the_runner_is_unreachable(self, plugin_module, monkeypatch):
        # Sem fallback pro Apollo direto - ele nao tem prep-cmd nenhum,
        # chamar ele sem o Runner so' derrubaria a conexao sem restaurar a
        # tela nem matar o jogo.
        await _save_config(plugin_module)
        monkeypatch.setattr(plugin_module, "RunnerClient", FakeRunnerClientUnreachable)

        result = await plugin_module.Plugin().stop_stream()

        assert result["ok"] is False
        assert "Runner" in result["error"]

    async def test_requires_a_configured_host_before_trying_anything(self, plugin_module):
        result = await plugin_module.Plugin().stop_stream()

        assert result == {"ok": False, "error": "Configure o host do Apollo primeiro"}


class TestRunnerClientCloseSession:
    async def test_sends_a_post_to_session_close_and_parses_the_json_response(self, plugin_module):
        # Servidor HTTP real (nao mockado) rodando numa porta livre local -
        # confirma o comportamento de verdade do cliente (metodo, path,
        # parsing da resposta), mesmo espirito do resto do projeto (testar
        # contra o SO/rede real quando viavel em vez de so' mockar).
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
