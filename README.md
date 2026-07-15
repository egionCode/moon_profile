# MoonProfile

Moonlight/Apollo streaming from the Steam Deck with context-based
profiles (docked vs handheld), without having to reconfigure
bitrate/resolution/HDR by hand every session.

Two components:

- **`moon_profile_decky/`** - [Decky Loader](https://decky.xyz/) plugin
  that runs on the Steam Deck.
- **`moon_profile_runner/`** - daemon that runs on the host PC (where
  Apollo is installed).

## Requirements

- **Host** (the PC that will be streamed):
  - [Apollo](https://github.com/ClassicOldSong/Apollo) 0.4.8+ configured
    and running.
  - Linux with KDE Plasma 6 on Wayland.
  - GPU with VAAPI encode support (tested with AMD RDNA).
  - `ydotool` + `ydotoold` installed, with
    `systemctl --user enable --now ydotool.service` - only needed if
    you're using a profile's "move cursor to corner" option.
- **Steam Deck** (or any client with Decky Loader):
  - [Decky Loader](https://wiki.deckbrew.xyz/en/plugin-dev/getting-started)
    installed.
  - [Moonlight Flatpak](https://flathub.org/apps/com.moonlight_stream.Moonlight)
    client (`com.moonlight_stream.Moonlight`) installed.

## Installation

### 1. Runner (on the host)

Via AUR:

```bash
yay -S moon-profile-runner-git
```

This builds, installs, and registers the graphical-session autostart.
Without `yay`/AUR, you can build it manually:

```bash
git clone https://github.com/egionCode/moon_profile.git
cd moon_profile/moon_profile_runner
./install.sh
```

### 2. Plugin (on the Steam Deck)

Download the zip from the [latest release](https://github.com/egionCode/moon_profile/releases/latest)
(`moonprofile-decky-*.zip`) and extract it into
`/home/deck/homebrew/plugins/`, then restart Decky Loader:

```bash
ssh deck@STEAMDECK_IP "systemctl --user restart plugin_loader"
```

Enable the "MoonProfile" plugin in the Decky Loader menu (satellite icon
in Quick Access).

### 3. Initial setup

In the plugin's "Settings" tab (Quick Access → MoonProfile → gear icon):

1. **Apollo config**: host IP, admin username and password for Apollo.
2. **Runner**: Runner port (default `47991`).
3. **Profiles**: edit the example profiles or create your own,
   resolution/fps/bitrate/codec/HDR on the Moonlight side; target
   monitor and monitors to disable on the host side.
4. In Quick Access, click "Sync games from host", this creates a
   library shortcut for each Steam game installed on the host, with
   cover/hero artwork downloaded automatically.

To play, just click "Play" on one of those shortcuts like you would on
any game.

## How it works

```
[Deck: clicks "Play" on a synced shortcut]
    ↓
Detects context (docked/handheld) and picks the matching profile
    ↓
Configures Apollo and notifies the Runner, which turns on the host
display (right monitor, resolution, HDR)
    ↓
Moonlight client connects and the stream comes up
    ↓
[game closes, on its own or via "Close connection" in Quick Access]
    ↓
Runner notices the game ended, notifies Apollo (Deck disconnects right
away) and restores the host display
```

## License

GPL-3.0, see [`LICENSE`](LICENSE).
