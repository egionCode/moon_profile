"""
main.py roda dentro do Decky Loader de verdade, que injeta py_modules/ no
sys.path e um modulo global "decky" com constantes de diretorio + logger
(ver decky-loader/sandboxed_plugin.py) - nada disso existe fora do loader.
Este conftest recria os dois artificialmente antes de importar main.py,
igual runner.py faz sozinho em produção (insere py_modules/ manualmente
no sys.path - ver runner.py).
"""

import sys
import types
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "py_modules"))
sys.path.insert(0, str(ROOT))


def _make_fake_decky_module(settings_dir: Path, plugin_dir: Path) -> types.ModuleType:
    fake = types.ModuleType("decky")
    fake.DECKY_PLUGIN_SETTINGS_DIR = str(settings_dir)
    fake.DECKY_PLUGIN_DIR = str(plugin_dir)
    fake.DECKY_PLUGIN_LOG = str(settings_dir / "plugin.log")

    class _Logger:
        def info(self, *args, **kwargs):
            pass

        def error(self, *args, **kwargs):
            pass

    fake.logger = _Logger()
    return fake


import pytest


@pytest.fixture
def plugin_module(tmp_path, monkeypatch):
    """
    Importa main.py com decky.DECKY_PLUGIN_SETTINGS_DIR/DECKY_PLUGIN_DIR
    apontando pra uma pasta temporaria isolada (nao a config de verdade do
    usuario) - cada teste comeca do zero, sem nenhum config.json/
    game_shortcuts.json pre-existente.
    """
    settings_dir = tmp_path / "settings"
    plugin_dir = tmp_path / "plugin"
    settings_dir.mkdir()
    plugin_dir.mkdir()

    fake_decky = _make_fake_decky_module(settings_dir, plugin_dir)
    monkeypatch.setitem(sys.modules, "decky", fake_decky)

    # Reimporta do zero a cada teste (nao reusa o modulo em cache doutro
    # teste) - senao o "decky" capturado no import ficaria preso na
    # primeira pasta temporaria criada na sessao toda.
    sys.modules.pop("main", None)
    import main as main_module

    return main_module
