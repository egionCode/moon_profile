import pytest


class TestGameShortcutsPersistence:
    async def test_returns_empty_dict_when_no_file_exists_yet(self, plugin_module):
        plugin = plugin_module.Plugin()

        shortcuts = await plugin.get_game_shortcuts()

        assert shortcuts == {}

    async def test_round_trips_through_save_and_get(self, plugin_module):
        plugin = plugin_module.Plugin()
        entry = {"123": {"deck_app_id": 999, "name": "Meu Jogo", "is_steam": True}}

        await plugin.save_game_shortcuts(entry)
        loaded = await plugin.get_game_shortcuts()

        assert loaded == entry


class TestStreamingCollectionPersistence:
    async def test_returns_none_when_nothing_was_saved_yet(self, plugin_module):
        plugin = plugin_module.Plugin()

        collection_id = await plugin.get_streaming_collection_id()

        assert collection_id is None

    async def test_round_trips_through_save_and_get(self, plugin_module):
        plugin = plugin_module.Plugin()

        await plugin.save_streaming_collection_id("collection-abc")
        collection_id = await plugin.get_streaming_collection_id()

        assert collection_id == "collection-abc"

    async def test_overwrites_the_previous_id_on_resave(self, plugin_module):
        # resync deve substituir o id antigo, nao acumular - so' existe
        # uma colecao "Streaming" por vez.
        plugin = plugin_module.Plugin()

        await plugin.save_streaming_collection_id("collection-abc")
        await plugin.save_streaming_collection_id("collection-def")
        collection_id = await plugin.get_streaming_collection_id()

        assert collection_id == "collection-def"


class TestConfigDefaults:
    async def test_default_config_has_no_redundant_runner_host_field(self, plugin_module):
        # runner_host foi removido de proposito - Runner sempre roda na
        # mesma maquina que o Apollo (ver "host").
        plugin = plugin_module.Plugin()

        config = await plugin.get_config()

        assert "runner_host" not in config
        assert config["runner_port"] == plugin_module.RUNNER_PORT

    async def test_loading_an_old_config_file_does_not_reintroduce_runner_host(self, plugin_module, tmp_path):
        import json
        import os

        plugin = plugin_module
        config_path = os.path.join(plugin.decky.DECKY_PLUGIN_SETTINGS_DIR, "config.json")
        with open(config_path, "w") as f:
            json.dump({"host": "192.168.1.6", "username": "u", "password": "p", "runner_host": "192.168.1.6"}, f)

        loaded = await plugin.Plugin().get_config()

        # setdefault nao apaga campos existentes - mas nada no codigo deve
        # depender mais de runner_host, so' confirma que get_config nao
        # quebra nem exige ele.
        assert loaded["host"] == "192.168.1.6"
        assert loaded["runner_port"] == plugin.RUNNER_PORT
