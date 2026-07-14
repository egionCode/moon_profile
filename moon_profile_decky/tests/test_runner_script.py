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
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "py_modules"))

_spec = importlib.util.spec_from_file_location("runner_script", ROOT / "runner" / "runner.py")
runner_script = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(runner_script)


class TestRegisterWithRunner:
    def test_posts_app_id_and_credentials_to_session_register(self):
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
            runner_script.register_with_runner(config, "2050650")
        finally:
            thread.join(timeout=5)
            server.server_close()

        assert received["path"] == "/session/register"
        assert received["body"] == {"app_id": "2050650", "username": "u", "password": "p"}

    def test_does_not_raise_when_the_runner_is_unreachable(self):
        # Best-effort de proposito - o Runner e' opcional, uma falha aqui
        # NAO pode impedir o jogo de rodar (ver comentario na funcao).
        config = {"host": "127.0.0.1", "username": "u", "password": "p", "runner_port": 1}

        runner_script.register_with_runner(config, "2050650")  # nao deve levantar excecao
