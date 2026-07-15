"""
runner.py roda como processo solto (exec'ado pela Steam), fora do runtime
do Decky Loader - importa direto do arquivo em vez de via um pacote
formal (nao tem __init__.py em runner/).
"""

import http.server
import importlib.util
import json
import sys
import threading
import urllib.error
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "py_modules"))

_spec = importlib.util.spec_from_file_location("runner_script", ROOT / "runner" / "runner.py")
runner_script = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(runner_script)


_PROFILE = {
    "host": {"target_output": "HDMI-A-1", "resolution": "1920x1080", "fps": 60, "hdr": False, "disable_outputs": []}
}


class TestRegisterWithRunner:
    def test_posts_app_id_credentials_display_and_restore_commands_to_session_register(self):
        received = {}

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                length = int(self.headers.get("Content-Length", 0))
                received["path"] = self.path
                received["body"] = json.loads(self.rfile.read(length))
                self.send_response(200)
                self.end_headers()

            def log_message(self, *args):
                pass

        server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        port = server.server_address[1]
        thread = threading.Thread(target=server.handle_request)
        thread.start()
        try:
            config = {"host": "127.0.0.1", "username": "u", "password": "p", "runner_port": port}
            runner_script.register_with_runner(config, "2050650", _PROFILE)
        finally:
            thread.join(timeout=5)
            server.server_close()

        assert received["path"] == "/session/register"
        body = received["body"]
        assert body["app_id"] == "2050650"
        assert body["username"] == "u"
        assert body["password"] == "p"
        assert body["display_commands"][0] == "kscreen-doctor output.HDMI-A-1.enable"
        assert body["restore_commands"][0] == "setsid steam steam://close/bigpicture"

    def test_raises_when_the_runner_is_unreachable(self):
        # O Runner NAO e' mais opcional - o Apollo nao tem prep-cmd
        # nenhum, entao sem o Runner a tela nunca troca. main() aborta o
        # lancamento quando isso levanta (ver comentario na funcao).
        config = {"host": "127.0.0.1", "username": "u", "password": "p", "runner_port": 1}

        with pytest.raises((urllib.error.URLError, OSError)):
            runner_script.register_with_runner(config, "2050650", _PROFILE)
