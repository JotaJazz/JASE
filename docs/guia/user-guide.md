**English** | [Español](guia-usuario.md)

# JASE user guide

Everything JASE can do, in detail. For a quick overview see the [README](../../README.md); for each game's masters and notes, [Games and masters](games-and-masters.md).

## Download

Get the latest version from the [Releases page](https://github.com/JotaJazz/JASE/releases). JASE checks GitHub at most once a day (when it starts and then every hour) and shows a link in the status bar when a newer version exists (you can turn this off in Settings → General); see [Updating from the app](#updating-from-the-app). There are two downloads:

- **Installer** (`JASE-<version>-setup.exe`, recommended): adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **Portable zip** (`JASE-<version>-win64.zip`): no install; unzip it anywhere and run `JASE.exe`.

Either way, settings are stored in `%APPDATA%\JimmysASE`, and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may show a warning the first time. Click **More info → Run anyway**.

## Usage

On first launch JASE offers to find your installed games. You can also open **File → Settings** and set each game's folder by hand (the one containing, per game: `CoD2MP_s.exe` for CoD2, `CoDMP.exe` for CoD, `CoDUOMP.exe` for United Offensive, `iw3mp.exe` for CoD4, `quake3.exe` for Quake III, `WolfMP.exe` for RtCW, `ET.exe` for Enemy Territory, `System\UnrealTournament.exe` for UT99, `System\TacticalOps.exe` for Tactical Ops (or the folder holding `TO340\` and `TO350\`), `MOHAA.exe` for Medal of Honor, `moh_spearhead.exe` for Spearhead and `moh_breakthrough.exe` for Breakthrough (see [Games and masters](games-and-masters.md#medal-of-honor-and-openmohaa)), `GameData\jk2mp.exe` for Jedi Outcast, `GameData\jamp.exe` for Jedi Academy, `SoF2MP.exe` for SoF2, `Quake3-UrT.exe` for Urban Terror, `openarena.exe` for OpenArena, `xonotic.exe` for Xonotic, `warsow_x64.exe` for Warsow, `warfork_x64.exe` for Warfork, `tremulous.exe` for Tremulous, `smokinguns.exe` for Smokin' Guns, `hl.exe` for CS 1.6, HL DM, DoD and TFC (Source games such as L4D, L4D2, CS: Source, TF2 and Garry's Mod connect through `steam://connect` and need no folder; their executables are `left4dead.exe`, `left4dead2.exe`, `hl2.exe`, `tf_win64.exe` and `hl2.exe` or `bin\win64\gmod.exe`), `BF1942.exe` for BF1942, `quake2.exe` for Quake II, the Quake II folder that holds `dday\` for D-Day (`quake2.exe`, `q2pro.exe`, `r1q2.exe` or `yquake2.exe`), `System\UT2004.exe` for UT2004 or `zandronum.exe` for Zandronum).

Pick a game in the left panel, press **Refresh list** (toolbar icon or `F5`) and double-click a server to join.

- **Quick filters** (left panel): *Hide empty*, *Hide full*, *Hide password-protected*, *Hide bots only*, *Max ping* and *Country* (the countries in the loaded list, with how many servers each has); they combine with the text and version filters and are remembered between sessions. The *Players* column shows people and bots apart (`2+6/16`: 2 people and 6 bots).
- **Shortcuts:** `Enter` connects to the selected server, `Ctrl+C` copies its IP:port (both with the table focused), `Ctrl+F` focuses the text filter, `Ctrl+Shift+F` searches for a player across the loaded lists and `F5` refreshes.
- **Context menu** (right-click a server): Connect, Copy IP:port, Refresh this server, Add/Remove from favorites and Notify me when there is room; in Recent, also Remove from recent.
- **Server card:** selecting a server shows its map image, country, players and rules.
- **Favorites:** the star on the card (or the context menu) marks a server as a favorite; **★ Favorites** in the left panel lists them per game, querying them directly (no master) and showing the ones that do not answer as offline.
- **Add server:** **Server → Add server…** (`Ctrl+N`, also at the bottom of the context menu) opens a dialog with game, IP and port; pasting `IP:port` (or a `jase://` link) fills the port by itself, and if the clipboard already holds an address it comes filled in. **Ctrl+V** on the server table opens it with what you copied. Pasting several addresses (one per line, or mixed with text from Discord or a forum) switches to a list and adds them all at once. **Detect** queries the address with every protocol and picks the game that answers (if several fit, it lists them). JASE queries the servers and selects the first one. Opened from **★ Favorites** (or right-clicking it in the left panel) they are saved as favorites, even if they do not answer. For GameSpy games (UT, MOHAA, BF1942) the game port works too: JASE also tries the matching query port.
- **Recent:** **🕘 Recent** in the left panel lists the servers you connected to (up to 20 per game, most recent first), queried directly like favorites; right-click → *Remove from recent* removes one.
- **Favorite groups:** right-click a favorite → **Group** → *New group…* (or an existing one) sorts them into groups such as “Clan”, “LAN” or “To try”. In **★ Favorites** a group selector shows up in the toolbar: it shows (and queries) only that group, and *Copy favorites as text* copies only what is shown. Groups are included when exporting favorites.
- **Favorite notes:** right-click a favorite (or **Server → Edit note…**) to write a note about it; it shows in the server card and in the tooltip of the name.
- **Export / import favorites:** **File → Export favorites…** / **Import favorites…** save and load them as a JSON file (notes included). Importing merges with your current favorites and discards invalid entries. **File → Copy favorites as text** (or right-click **★ Favorites**) copies the current game's favorites as `IP:port  name` lines to paste in Discord; pasted back into JASE they are added again.
- **Saved filters:** the **Saved filters** button under the quick filters saves the current combination under a name, applies a saved one or deletes it.
- **Notify me when there is room:** on a full server, right-click (or **Server** menu) → *Notify me when there is room* queries it every 10 s for up to 30 min and, when a slot frees up, asks whether to connect. Only one server is watched at a time; choose *Stop watching* to cancel.
- **Notify me when a game gets going:** right-click a server (or **Server** menu) → *Notify me when a game gets going…* asks how many players you are waiting for (4 by default; 0 turns it off). JASE checks it in the background every 2 minutes and notifies you once when it gets there; only people count, not bots. It notifies again if the server drops to half and fills up again.
- **Game tree and toolbar:** the left panel is a tree per game with its **Mods** and **Maps** (ticking them filters the table; *Clear mod/map filters* resets them). The toolbar and menu give access to refresh, stop, connect, favorites, settings and theme; *Find games…* is in the **File** menu.
- **Local network (LAN):** **Server → Search the local network** (`Ctrl+L`) looks for servers of the current game on your network (home, LAN party, internet café) without the master: it broadcasts the game's query to its usual ports and lists those that answer. That list does not replace the internet one; `F5` brings it back.
- **Playing now:** **Server → Playing now…** (`Ctrl+J`) gathers the servers with people from all your ready games (with a folder set, or joining through Steam) into one list, busiest first and not counting bots. It reuses the lists already loaded this session and scans the rest in the background (three games at a time); *Refresh* scans them all again. Double-click a row to show that server in its game.
- **RCON console** (for server admins): **Server → RCON console…** (`Ctrl+Shift+R`) sends admin commands to the selected server (change map, kick, `status`…) and shows the answer; up/down arrows walk the history. It works with Quake 3 and Quake II games, Half-Life 1 (CS 1.6, DoD, TFC, HL DM) and Source games (CS: Source, TF2, L4D, Garry's Mod; RCON over TCP on the game port). The password can be saved in the Windows Credential Manager.
- **Friends:** **Server → Friends…** (`Ctrl+Shift+A`) manages a list of player names; you can also use *Add to friends* in the players table context menu. Friends appear in bold in the players table and JASE notifies you when one shows up on a server you query. Optionally (in the same dialog) it checks your favorites and recent servers in the background every 2 minutes.
- **System tray:** with **Settings → General → Minimize to the system tray** the window goes to the tray instead of the taskbar; notifications (free slot, friends) are then shown from the tray.
- **jase:// links:** *Copy jase:// link* in the context menu copies a link to the server. Opening such a link starts JASE (or uses the running instance), queries the server and asks before connecting. The installer registers the protocol; the portable zip does not.
- **Occupancy history:** favorites and recent servers show in the card an hourly average chart of players (last 14 days, stored in `%APPDATA%\JimmysASE\cache\history.json`).
- **Window:** size, splitters, columns and the last game are remembered.

### Find games

**Find games…** (**File** menu or **File → Settings**) detects installed games in the background: Windows registry, Steam libraries and common folders (`Program Files`, `Games`, `Juegos`…). **Search in folder…** also scans a folder of your choice. Each game is shown as *new*, *already set up* or *saved path not found*. A path that already works is never replaced unless you tick its checkbox, and only ticked rows are applied. On the very first launch, with no games configured, it offers to search once.

### Masters

The official master list is updated automatically from GitHub (at most once a day; a bad download is ignored and the last good list is kept). Your own masters live in `masters.custom.json`, next to `config.json`. Edit them in **File → Settings → Masters**: add masters (`host:port`), remove your own, untick official ones, test them and check for list updates by hand.

### Automatic refresh

In **File → Settings → Refresh automatically** you can make the list reload by itself every 2, 5, 10 or 15 minutes (off by default). It does nothing while a scan is running, a dialog is open or the window is minimized, keeps the selected server selected and, in Favorites or Recent, re-queries those servers directly without using the master.

### Updating from the app

On Windows, clicking *New version X available* in the status bar offers to download and run the installer (its SHA-256 is verified against the one published on GitHub) or to open the release page. In the portable zip, *Download and install* gets the new zip (also verified), unpacks it, closes JASE, replaces the folder and opens it again; settings are untouched because they live in `%APPDATA%`. **Help → Check for updates…** asks GitHub right away (even with the automatic notice turned off) and tells you if you already have the latest version.

### Language

The interface is available in English, Spanish, Portuguese (Brazil), French, German and Italian. By default it follows your Windows language; you can pick another one in **File → Settings → Language**. The change takes effect after restarting JASE.

### Steam games and the Steam Web API key

Left 4 Dead, Left 4 Dead 2, Counter-Strike: Source, Team Fortress 2, Half-Life Deathmatch, Day of Defeat, Team Fortress Classic and Garry's Mod are listed through the Steam Web API, which needs a free key:

1. Get it at https://steamcommunity.com/dev/apikey. Any domain works (e.g. `localhost`). Per Steam's rules the account needs Steam Guard and a minimum spend.
2. Paste it in **File → Settings → General → Steam Web API key**.

The key is stored in the Windows Credential Manager (service `JimmysAllSeeingEye`), never in `config.json`. Without it, Steam games show a hint instead of a server list. By default only non-empty servers are listed; **Settings → General → Include empty Steam servers** also lists the empty ones (a much longer list). L4D, L4D2, CS: Source, TF2 and Garry's Mod connect through Steam (`steam://connect`), so they need no install folder; HL DM, DoD and TFC are launched with `hl.exe`, like CS 1.6.

## More details

### Map images

- **CoD2:** read from the installed game's `.iwd` files (verified). The other CoD games try the same, with no guarantee. Quake 3, Open Arena, RtCW, Enemy Territory, Urban Terror, Jedi Outcast, Jedi Academy, Soldier of Fortune II, Warsow, Warfork, Tremulous and Smokin' Guns read `levelshots/` from the `.pk3` files of their game folder, with no guarantee. Jedi Outcast, Jedi Academy, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns have no images on GameTracker (probed 2026-10-24); Xonotic shows no map image.
- **UT99 and Tactical Ops:** generated from the `.unr` files in the game's `Maps` folder.
- **GameTracker (optional, off by default):** used by CoD, CoD United Offensive, CoD4, Quake 3, Open Arena, RtCW, Enemy Territory, Urban Terror, SoF2, UT99, UT2004, MOHAA and its expansions, Counter-Strike 1.6, Counter-Strike: Source, Team Fortress 2 and Classic, Left 4 Dead 1 and 2, Half-Life Deathmatch, Day of Defeat, Garry's Mod, Battlefield 1942, Quake II and D-Day (its `dday1`…`dday5` maps are in GameTracker's Quake II folder). RtCW has partial coverage: its folder is Enemy Territory's and only holds the maps both games share. Jedi Academy uses local files only. In **File → Settings** you can enable downloading images from `image.gametracker.com` for maps that are not installed. They are stored in `%APPDATA%\JimmysASE\cache\maps`. Without this option JASE downloads no images.

### Server country

The **Country** column shows each server's flag (name in the tooltip and the card), and the text filter accepts the country code (`cl`, `br`…). The country is looked up offline from the server's IPv4 in DB-IP's "IP to Country Lite" database. The first time JASE runs (and while the file is missing) it is downloaded once in the background to `%APPDATA%\JimmysASE\cache\dbip-country-lite.csv` (~4.5 MB compressed, ~30 MB on disk). Without a network the column stays empty and only the status bar says so. **Settings → Update country database** downloads it again (DB-IP publishes it monthly).

### Configuration and passwords

- Configuration (game paths, theme, version filter, quick filters, saved filters, favorites and their notes, friends, tray option, window and masters) is stored in `%APPDATA%\JimmysASE\config.json`. You can edit it by hand with the app closed. Your own masters (added or disabled in *Settings → Masters*) live in `masters.custom.json` next to it.
- Server passwords are **not** saved in that file: they go to the **Windows Credential Manager** (service `JimmysAllSeeingEye`). You can view or delete them in *Control Panel → Credential Manager → Windows Credentials*.
- Cache in `%APPDATA%\JimmysASE\cache\`: map images (`maps\`), the country database (`dbip-country-lite.csv`), the occupancy history (`history.json`), the last server list of each game (`servers\`), the downloaded masters list (`masters.json`) and update downloads (`updates\`).

## Install and run from source

Requirements: Windows 10/11 and **Python 3.11 or newer for Windows** (from [python.org](https://www.python.org/downloads/windows/); tick "Add python.exe to PATH" and keep the `py` launcher). Python dependencies of the app (the `gui` extra, installed by `pip install -e ".[gui]"`): PySide6, keyring and Pillow (reads map images from the game files).

From the project folder, in PowerShell:

```powershell
py -m venv .venv
.venv\Scripts\pip install -e ".[gui]"
.venv\Scripts\python -m jase
```

(The `.venv\Scripts\jase` command is also available.) On Linux it opens the Windows games with Wine (the command is set in *Settings → General → Open games with*, e.g. `wine` or `umu-run`; experimental); under WSL it runs for development.

### Command line without a GUI

`jase-cli` runs the same queries without the interface, for scripts, servers or panels. `pip install .` (no extras) installs only the query engine, which uses the standard library alone:

```sh
pip install .
jase-cli games
jase-cli scan cod2 --country
jase-cli scan l4d2 --json --steam-key YOUR_KEY   # or the JASE_STEAM_KEY variable
```

With `--json` the output follows a versioned contract (`"schema": 1`) with structured errors per master; see [docs/cli.md](../cli.md).

### Tests

```powershell
.venv\Scripts\pip install -e ".[dev]"
.venv\Scripts\python -m pytest
```

To check that the real masters answer (queries every listed server; for a light run use `--sample N` and, optionally, game ids):

```powershell
.venv\Scripts\python scripts\probe_masters.py --sample 10 cod2 ut99
.venv\Scripts\python scripts\probe_masters.py
```

Architecture, conventions and contributor notes (in Spanish): [docs/README.md](../README.md).

## Credits

- IP to Country Lite by [DB-IP](https://db-ip.com), CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
- Flags: [flag-icons](https://github.com/lipis/flag-icons) 7.5.0 by Panayiotis Lipiridis, MIT license (`src/jase/assets/flags/LICENSE`).
- Built with PySide6 (Qt for Python, LGPL-3.0), keyring and Pillow.
- Community masters and projects that keep these games alive: 333networks, OpenSpy, jkhub, 1fxmod, dpmaster and cs16.net.
