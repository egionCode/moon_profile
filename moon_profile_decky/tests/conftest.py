"""
main.py runs inside the real Decky Loader, which injects py_modules/ into
sys.path and a global "decky" module with directory constants + a logger
(see decky-loader/sandboxed_plugin.py): none of that exists outside the
loader. This conftest artificially recreates both before importing
main.py, the same way runner.py does on its own in production (manually
inserting py_modules/ into sys.path, see runner.py).
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
    Imports main.py with decky.DECKY_PLUGIN_SETTINGS_DIR/DECKY_PLUGIN_DIR
    pointing to an isolated temporary folder (not the user's real config),
    so each test starts from scratch, with no pre-existing config.json/
    game_shortcuts.json.
    """
    settings_dir = tmp_path / "settings"
    plugin_dir = tmp_path / "plugin"
    settings_dir.mkdir()
    plugin_dir.mkdir()

    fake_decky = _make_fake_decky_module(settings_dir, plugin_dir)
    monkeypatch.setitem(sys.modules, "decky", fake_decky)

    # Reimports from scratch on every test (doesn't reuse the module cached
    # from another test), otherwise the "decky" captured at import time
    # would stay stuck on the first temporary folder created in the whole session.
    sys.modules.pop("main", None)
    import main as main_module

    return main_module
