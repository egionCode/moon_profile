import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "py_modules"))

from moonprofile_core import build_display_commands, build_restore_commands, classify_apollo_error, detect_context
import json
import urllib.error


def _write_drm_output(drm_root: Path, name: str, status: str) -> None:
    entry = drm_root / name
    entry.mkdir(parents=True)
    (entry / "status").write_text(status)


class TestDetectContext:
    def test_handheld_when_only_internal_display_is_connected(self, tmp_path):
        drm_root = tmp_path / "drm"
        _write_drm_output(drm_root, "card0-eDP-1", "connected")

        assert detect_context(str(drm_root)) == "handheld"

    def test_docked_when_an_external_display_is_connected(self, tmp_path):
        drm_root = tmp_path / "drm"
        _write_drm_output(drm_root, "card0-eDP-1", "connected")
        _write_drm_output(drm_root, "card0-HDMI-A-1", "connected")

        assert detect_context(str(drm_root)) == "docked"

    def test_handheld_when_external_output_exists_but_disconnected(self, tmp_path):
        # regressao real: "eDP-1" contem "DP" como substring - sem excluir
        # eDP explicitamente, isso sempre voltava "docked".
        drm_root = tmp_path / "drm"
        _write_drm_output(drm_root, "card0-eDP-1", "connected")
        _write_drm_output(drm_root, "card0-HDMI-A-1", "disconnected")

        assert detect_context(str(drm_root)) == "handheld"

    def test_handheld_when_no_drm_outputs_exist_at_all(self, tmp_path):
        drm_root = tmp_path / "drm"
        drm_root.mkdir()

        assert detect_context(str(drm_root)) == "handheld"


class TestBuildDisplayCommands:
    def test_configures_the_target_output_in_order(self):
        host_cfg = {"target_output": "HDMI-A-1", "resolution": "1920x1080", "fps": 60, "hdr": False, "disable_outputs": []}

        commands = build_display_commands(host_cfg)

        assert commands == [
            "kscreen-doctor output.HDMI-A-1.enable",
            "kscreen-doctor output.HDMI-A-1.mode.1920x1080@60",
            "kscreen-doctor output.HDMI-A-1.hdr.disable output.HDMI-A-1.wcg.disable",
        ]

    def test_enables_hdr_when_the_profile_wants_it(self):
        host_cfg = {"target_output": "HDMI-A-1", "resolution": "1920x1080", "fps": 60, "hdr": True, "disable_outputs": []}

        commands = build_display_commands(host_cfg)

        assert "kscreen-doctor output.HDMI-A-1.hdr.enable output.HDMI-A-1.wcg.enable" in commands

    def test_disables_the_other_outputs_after_configuring_the_target(self):
        host_cfg = {
            "target_output": "HDMI-A-1",
            "resolution": "1920x1080",
            "fps": 60,
            "hdr": False,
            "disable_outputs": ["DP-2", "DP-3"],
        }

        commands = build_display_commands(host_cfg)

        assert commands[3:] == ["kscreen-doctor output.DP-2.disable", "kscreen-doctor output.DP-3.disable"]

    def test_does_not_move_the_cursor_by_default(self):
        host_cfg = {"target_output": "HDMI-A-1", "resolution": "1920x1080", "fps": 60, "hdr": False, "disable_outputs": []}

        commands = build_display_commands(host_cfg)

        assert not any("ydotool" in c for c in commands)

    def test_moves_the_cursor_to_the_bottom_right_corner_when_enabled(self):
        # achado real: alguns jogos (FIFA) prendem o cursor no meio da
        # tela mesmo jogando so' de controle - ydotool e' o unico jeito de
        # mover o cursor no Wayland sem apoio do compositor (KWin nao
        # deixa escrever workspace.cursorPos nessa versao do Plasma).
        host_cfg = {
            "target_output": "HDMI-A-1",
            "resolution": "1920x1080",
            "fps": 60,
            "hdr": False,
            "disable_outputs": [],
            "move_cursor_to_corner": True,
        }

        commands = build_display_commands(host_cfg)

        assert commands[-1] == "ydotool mousemove -a 1919 1079"

    def test_does_not_enter_bigpicture_by_default(self):
        host_cfg = {"target_output": "HDMI-A-1", "resolution": "1920x1080", "fps": 60, "hdr": False, "disable_outputs": []}

        commands = build_display_commands(host_cfg)

        assert not any("bigpicture" in c for c in commands)

    def test_enters_bigpicture_when_enabled(self):
        host_cfg = {
            "target_output": "HDMI-A-1",
            "resolution": "1920x1080",
            "fps": 60,
            "hdr": False,
            "disable_outputs": [],
            "enter_bigpicture": True,
        }

        commands = build_display_commands(host_cfg)

        assert "setsid steam steam://open/bigpicture" in commands


class TestBuildRestoreCommands:
    def test_does_not_close_bigpicture_by_default(self):
        # so' faz sentido fechar o Big Picture no fechamento se o perfil
        # de fato abriu ele no lancamento (enter_bigpicture) - ver
        # build_display_commands.
        host_cfg = {"target_output": "HDMI-A-1", "disable_outputs": []}

        commands = build_restore_commands(host_cfg)

        assert not any("bigpicture" in c for c in commands)

    def test_closes_big_picture_first_then_settles_when_enabled(self):
        host_cfg = {"target_output": "HDMI-A-1", "disable_outputs": [], "enter_bigpicture": True}

        commands = build_restore_commands(host_cfg)

        assert commands[0] == "setsid steam steam://close/bigpicture"
        assert commands[1] == "sleep 2"

    def test_has_no_pkill_or_long_sleep_steps(self):
        # diferenca chave vs build_prep_cmd: nada de matar processo nem
        # sleep 20 - o jogo ja foi confirmado morto antes de chamar isso
        # (watchdog do Runner), o periodo de graca nao serve pra nada aqui.
        host_cfg = {"target_output": "HDMI-A-1", "disable_outputs": ["DP-2"]}

        commands = build_restore_commands(host_cfg)

        assert not any("pkill" in c for c in commands)
        assert "sleep 20" not in commands

    def test_does_not_disable_target_output_without_a_fallback_display(self):
        host_cfg = {"target_output": "HDMI-A-1", "disable_outputs": []}

        commands = build_restore_commands(host_cfg)

        assert not any("HDMI-A-1.disable" in c for c in commands)

    def test_re_enables_disabled_outputs_in_order_then_disables_the_target(self):
        host_cfg = {"target_output": "HDMI-A-1", "disable_outputs": ["DP-2", "DP-3"], "enter_bigpicture": True}

        commands = build_restore_commands(host_cfg)

        assert commands == [
            "setsid steam steam://close/bigpicture",
            "sleep 2",
            "kscreen-doctor output.DP-2.enable",
            "kscreen-doctor output.DP-3.enable",
            "sleep 1",
            "kscreen-doctor output.HDMI-A-1.disable",
        ]


class TestClassifyApolloError:
    def test_wrong_credentials_reported_as_401(self):
        error = urllib.error.HTTPError("url", 401, "Unauthorized", {}, None)

        message = classify_apollo_error("192.168.1.6", error)

        assert "senha" in message.lower()

    def test_unexpected_http_status_includes_the_code(self):
        error = urllib.error.HTTPError("url", 500, "Internal Server Error", {}, None)

        message = classify_apollo_error("192.168.1.6", error)

        assert "500" in message

    def test_non_json_response_mentions_the_host(self):
        try:
            json.loads("nao e json")
        except json.JSONDecodeError as error:
            message = classify_apollo_error("192.168.1.6", error)
            assert "192.168.1.6" in message
            return
        pytest.fail("json.loads deveria ter lancado JSONDecodeError")

    def test_unreachable_host_mentions_the_host(self):
        error = OSError("connection refused")

        message = classify_apollo_error("192.168.1.6", error)

        assert "192.168.1.6" in message
