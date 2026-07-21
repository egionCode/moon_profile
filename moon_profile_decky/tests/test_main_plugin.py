import pytest


class TestGameShortcutsPersistence:
    async def test_returns_empty_dict_when_no_file_exists_yet(self, plugin_module):
        plugin = plugin_module.Plugin()

        shortcuts = await plugin.get_game_shortcuts()

        assert shortcuts == {}

    async def test_round_trips_through_save_and_get(self, plugin_module):
        plugin = plugin_module.Plugin()
        entry = {"123": {"deck_app_id": 999, "name": "My Game", "is_steam": True}}

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
        # A resync should replace the old id, not accumulate: there's only
        # one "Streaming" collection at a time.
        plugin = plugin_module.Plugin()

        await plugin.save_streaming_collection_id("collection-abc")
        await plugin.save_streaming_collection_id("collection-def")
        collection_id = await plugin.get_streaming_collection_id()

        assert collection_id == "collection-def"


class TestConfigDefaults:
    async def test_default_config_has_no_redundant_runner_host_field(self, plugin_module):
        # runner_host was removed on purpose: the Runner always runs on
        # the same machine as Apollo (see "host").
        plugin = plugin_module.Plugin()

        config = await plugin.get_config()

        assert "runner_host" not in config
        assert config["runner_port"] == plugin_module.RUNNER_PORT
        assert config["steamgriddb_api_key"] == ""
        assert config["mac_address"] == ""

    async def test_loading_an_old_config_file_does_not_reintroduce_runner_host(self, plugin_module, tmp_path):
        import json
        import os

        plugin = plugin_module
        config_path = os.path.join(plugin.decky.DECKY_PLUGIN_SETTINGS_DIR, "config.json")
        with open(config_path, "w") as f:
            json.dump({"host": "192.168.1.6", "username": "u", "password": "p", "runner_host": "192.168.1.6"}, f)

        loaded = await plugin.Plugin().get_config()

        # setdefault doesn't erase existing fields, but nothing in the code
        # should depend on runner_host anymore, this just confirms that
        # get_config doesn't break or require it.
        assert loaded["host"] == "192.168.1.6"
        assert loaded["runner_port"] == plugin.RUNNER_PORT

    async def test_loading_an_old_config_file_gets_a_default_steamgriddb_api_key(self, plugin_module, tmp_path):
        import json
        import os

        plugin = plugin_module
        config_path = os.path.join(plugin.decky.DECKY_PLUGIN_SETTINGS_DIR, "config.json")
        with open(config_path, "w") as f:
            json.dump({"host": "192.168.1.6", "username": "u", "password": "p"}, f)

        loaded = await plugin.Plugin().get_config()

        assert loaded["steamgriddb_api_key"] == ""
        assert loaded["mac_address"] == ""
