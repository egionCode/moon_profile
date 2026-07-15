# MoonProfile

[Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader) plugin for Steam Deck that manages [Moonlight](https://moonlight-stream.org/)/[Apollo](https://github.com/ClassicOldSong/Apollo) streaming profiles with automatic context detection (docked vs handheld) and dynamic host configuration via Apollo's REST API.

## The problem this solves

Moonlight doesn't know whether the Deck is docked to a 4K TV or handheld on the internal screen, it always fires the same resolution/bitrate, and Apollo's prep-cmd is fixed, doesn't adapt to different scenarios (HDR on the TV vs SDR handheld). Configuring this manually every session doesn't scale.

MoonProfile detects the context automatically (via `/sys/class/drm`, checking whether any external display is connected) and applies a profile that controls, at the same time, the Moonlight client configuration (resolution, fps, bitrate, codec, HDR) and the host's display configuration via Apollo (active output, resolution, HDR/WCG, which outputs to disable).

## Difference from MoonDeck

- Zero additional component on the host, only talks to the REST API Apollo already exposes, no need to install/maintain a companion daemon (Buddy) running on the PC.
- Streaming profiles editable directly on the Deck, with automatic context detection (docked/handheld), MoonDeck doesn't have this.
- Each profile controls the Moonlight client and the host simultaneously.

## Requirements

- Apollo 0.4.8+ running on the host.
- KDE Plasma 6 (Wayland) on the host, display control uses `kscreen-doctor`.
- AMD RDNA 4 GPU or equivalent (VAAPI).
- Moonlight Flatpak (`com.moonlight_stream.Moonlight`) installed on the Deck.

## How to use

1. **Configure Apollo** (Quick Access → ⚙️ in the title → "Apollo config" tab): host, username and password, the same credentials as Apollo's web panel.
2. **Create profiles** (the "Profiles" tab): each profile has a trigger (`docked`, `handheld`, or `manual`), the Moonlight client configuration, and the host's display configuration. At least one profile with the `docked` trigger and one with `handheld` covers automatic detection.
3. **Sync games from the host** (Quick Access): creates a library shortcut for each Steam game installed on the host, with cover/hero art.
4. **Play**: click "Play" on a synced shortcut like any other game in the library. It detects the current context, applies the matching profile, configures Apollo via its API, and launches Moonlight through a Steam shortcut (needed so Gamescope focuses the window correctly).
5. **Close connection** (Quick Access): ends the session on Apollo, restores the host's displays to their original configuration.
6. **Logs** (the "Logs" tab in settings): shows the last lines of the plugin's current session log, no SSH needed.

## Development

```bash
pnpm i
pnpm run build       # frontend build (dist/index.js)
./deploy.sh           # syncs with the Deck and restarts plugin_loader
./deploy.sh build     # builds and syncs in one go
```

`deploy.sh` expects a passwordless SSH key for the Deck and a few
NOPASSWD `sudoers` rules, see the comments at the top of the script.

Full documentation (motivation, architecture, execution phases,
decisions, and known limitations) in [`docs/prd.md`](docs/prd.md).
