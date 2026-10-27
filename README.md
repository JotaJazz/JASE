[Español](README.es.md) | **English**

![Jimmy's All Seeing Eye](docs/banner.svg)

# Jimmy's All Seeing Eye (JASE)

A game server browser in the spirit of the classic *All-Seeing Eye*. It queries the master servers, lists servers with name, map, players, version and ping, and launches the game already connected with a double click.

![JASE main window](docs/screenshot.png)

## Features

- 35 supported games (Medal of Honor counts Spearhead and Breakthrough separately) on six protocols: Quake 2 (Quake II, D-Day: Normandy), Quake 3 (Call of Duty, Quake III, Wolfenstein, Jedi Outcast, Jedi Academy, SoF2, Urban Terror, OpenArena, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns), GameSpy (Unreal Tournament, Medal of Honor, Battlefield 1942), Valve (Counter-Strike 1.6; plus, through the Steam Web API, Left 4 Dead, Left 4 Dead 2, Counter-Strike: Source, Team Fortress 2, Half-Life Deathmatch, Day of Defeat, Team Fortress Classic and Garry's Mod) UT2004 (Unreal Tournament 2004) and Zandronum (Zandronum (Doom), multiplayer Doom).
- Server table with country flags; a server card with map image (from your installed games, optional online download), players and rules.
- Quick filters (hide empty, hide full, hide password-protected, hide bots only, max ping, country), plus text and version filters.
- Game tree with **Mods** and **Maps** you can tick to filter the table.
- Favorites, queried directly without a master.
- Automatic game detection: Windows registry, Steam libraries and common folders.
- Server passwords stored in the Windows Credential Manager.
- Eleven themes (Dark, Light, Neon, Brown, Green, Classic 98, Windows XP, Terminal, Synthwave, Rust, GoldSrc) plus System, chosen in **View → Theme** or *Settings → Appearance* (Terminal in green, white, sepia or amber phosphor) and applied instantly; English, Spanish, Portuguese, French, German and Italian interface.

## Supported games

Call of Duty 2, Call of Duty, Call of Duty: United Offensive, Call of Duty 4: Modern Warfare, Quake III Arena, Return to Castle Wolfenstein, Wolfenstein: Enemy Territory, Unreal Tournament (UT99), Medal of Honor: Allied Assault (with Spearhead and Breakthrough, also OpenMoHAA), Star Wars Jedi Knight II: Jedi Outcast, Star Wars Jedi Knight: Jedi Academy, Soldier of Fortune II, Urban Terror, OpenArena, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns, Counter-Strike 1.6, Battlefield 1942, Quake II, D-Day: Normandy (Quake II mod), Unreal Tournament 2004, Left 4 Dead, Left 4 Dead 2, Counter-Strike: Source, Team Fortress 2, Half-Life Deathmatch, Day of Defeat, Team Fortress Classic and Garry's Mod (the Steam ones need a Steam Web API key).

Masters were verified on 2026-10-18/19, and the Steam Web API and the UT2004 masters on 2026-10-20; see [Masters used](#masters-used).
Half-Life Deathmatch, Day of Defeat and Team Fortress Classic are listed through the Steam Web API (probed 2026-10-20), and Garry's Mod too (probed 2026-10-25: 1064 non-empty servers).
The executables and connection arguments for Jedi Academy, SoF2, CS 1.6, BF1942, Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns (`+connect`) and D-Day (`+set game dday`) are still pending verification on Windows.

## Download

Get the latest version from the [Releases page](https://github.com/JotaJazz/JASE/releases). JASE checks GitHub at most once a day (when it starts and then every hour) and shows a link in the status bar when a newer version exists (you can turn this off in Settings → General); see [Updating from the app](#updating-from-the-app). There are two downloads:

- **Installer** (`JASE-<version>-setup.exe`, recommended): adds a Start menu shortcut (desktop shortcut optional) and an uninstaller in *Settings → Apps*. It installs just for you without admin rights, or for all users if you choose.
- **Portable zip** (`JASE-<version>-win64.zip`): no install; unzip it anywhere and run `JASE.exe`.

Either way, settings are stored in `%APPDATA%\JimmysASE`, and uninstalling keeps them.

Neither the installer nor the exe is code-signed, so Windows SmartScreen may show a warning the first time. Click **More info → Run anyway**.

## Usage

On first launch JASE offers to find your installed games. You can also open **File → Settings** and set each game's folder by hand (the one containing, per game: `CoD2MP_s.exe` for CoD2, `CoDMP.exe` for CoD, `CoDUOMP.exe` for United Offensive, `iw3mp.exe` for CoD4, `quake3.exe` for Quake III, `WolfMP.exe` for RtCW, `ET.exe` for Enemy Territory, `System\UnrealTournament.exe` for UT99, `MOHAA.exe` for Medal of Honor (see [below](#medal-of-honor-and-openmohaa)), `GameData\jk2mp.exe` for Jedi Outcast, `GameData\jamp.exe` for Jedi Academy, `SoF2MP.exe` for SoF2, `Quake3-UrT.exe` for Urban Terror, `openarena.exe` for OpenArena, `xonotic.exe` for Xonotic, `warsow_x64.exe` for Warsow, `warfork_x64.exe` for Warfork, `tremulous.exe` for Tremulous, `smokinguns.exe` for Smokin' Guns, `hl.exe` for CS 1.6, HL DM, DoD and TFC (Source games such as L4D, L4D2, CS: Source, TF2 and Garry's Mod connect through `steam://connect` and need no folder; their executables are `left4dead.exe`, `left4dead2.exe`, `hl2.exe`, `tf_win64.exe` and `hl2.exe` or `bin\win64\gmod.exe`), `BF1942.exe` for BF1942, `quake2.exe` for Quake II, the Quake II folder that holds `dday\` for D-Day (`quake2.exe`, `q2pro.exe`, `r1q2.exe` or `yquake2.exe`), `System\UT2004.exe` for UT2004 or `zandronum.exe` for Zandronum).

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

On Windows, clicking *New version X available* in the status bar offers to download and run the installer (its SHA-256 is verified against the one published on GitHub) or to open the release page. The portable zip is updated by hand. **Help → Check for updates…** asks GitHub right away (even with the automatic notice turned off) and tells you if you already have the latest version.

### Language

The interface is available in English, Spanish, Portuguese (Brazil), French, German and Italian. By default it follows your Windows language; you can pick another one in **File → Settings → Language**. The change takes effect after restarting JASE.

### Steam games and the Steam Web API key

Left 4 Dead, Left 4 Dead 2, Counter-Strike: Source, Team Fortress 2, Half-Life Deathmatch, Day of Defeat, Team Fortress Classic and Garry's Mod are listed through the Steam Web API, which needs a free key:

1. Get it at https://steamcommunity.com/dev/apikey. Any domain works (e.g. `localhost`). Per Steam's rules the account needs Steam Guard and a minimum spend.
2. Paste it in **File → Settings → General → Steam Web API key**.

The key is stored in the Windows Credential Manager (service `JimmysAllSeeingEye`), never in `config.json`. Without it, Steam games show a hint instead of a server list. By default only non-empty servers are listed; **Settings → General → Include empty Steam servers** also lists the empty ones (a much longer list). L4D, L4D2, CS: Source, TF2 and Garry's Mod connect through Steam (`steam://connect`), so they need no install folder; HL DM, DoD and TFC are launched with `hl.exe`, like CS 1.6.

## Masters used

| Game | Master | Protocols / versions |
|---|---|---|
| Call of Duty 2 | `cod2master.activision.com:20710` | 115 = 1.0, 117 = 1.2, 118 = 1.3, 119/120 = CoD2x 1.4.x (joining needs the CoD2x client) |
| Call of Duty | `codmaster.activision.com:20510` | 1 = 1.1, 2 = 1.2, 4 = 1.3, 5 = 1.4, 6 = 1.5 |
| Unreal Tournament | `master.333networks.com:28900`, `master.openspy.net:28900` | 436, 451, 469 |
| Call of Duty: United Offensive | `coduomaster.activision.com:20610` | 21 = 1.41, 22 = 1.51, 24 = 1.51 |
| Call of Duty 4 | `cod4master.activision.com:20810` | 6 = 1.7 (and CoD4X, version "-"), 7 = 1.8, 5 = 1.6, 1 and 8 = 1.0 |
| Quake III Arena | `master.ioquake3.org:27950`, `master.quake3arena.com:27950`, `master.maverickservers.com:27950` | 68 = 1.32 (Q3 1.32e, ioq3 1.36, CNQ3 1.53) |
| Return to Castle Wolfenstein | `wolfmaster.idsoftware.com:27950` | 57, 60 = 1.41 (Wolf 1.0, Wolf 1.41/1.41b, iortcw 1.51) |
| Wolfenstein: Enemy Territory | `etmaster.idsoftware.com:27950`, `master.etlegacy.com:27950` | 84 = 2.60b / ET: Legacy |
| Medal of Honor: Allied Assault | `master.333networks.com:28900`, `master.openspy.net:28900` (gamename `mohaa`) | 1.11 (OpenMoHAA: 1.12+0.8x) |
| Medal of Honor: Spearhead | same (gamename `mohaas`) | 2.15 (OpenMoHAA: 2.16+0.8x) |
| Medal of Honor: Breakthrough | same (gamename `mohaab`) | 2.40 (OpenMoHAA: 2.41+0.8x) |
| Jedi Academy | `master.jkhub.org:29060` | 26 = 1.01 |
| Soldier of Fortune II | `master.1fxmod.org:20110` | 2002 = 1.00, 2004 = 1.03 |
| Urban Terror | `master.urbanterror.info:27900` | 68 (ioQ3 1.35 and mods; gamename `q3urt43`) |
| OpenArena | `dpmaster.deathmask.net:27950`, `master.ioquake3.org:27950` | 71 (ioq3+oa 1.36; gamename `baseoa`) |
| Jedi Outcast | `master.jkhub.org:28060` | 15 = 1.02, 16 = 1.04 (mostly JK2MV, `JK2MP: v1.04mv`) |
| Xonotic | `dpmaster.deathmask.net:27950`, `dpmaster.tchr.no:27950` (gamename `Xonotic`) | 3 (`gameversion` 806 = 0.8.6) |
| Warsow | `dpmaster.deathmask.net:27950`, `dpmaster.tchr.no:27950` (gamename `Warsow`) | 22 = 2.10 |
| Warfork | `dpmaster.deathmask.net:27950` (gamename `Warfork`) | 26 = 2.15 |
| Tremulous | `master.tremulous.net:30710` | 69 = 1.1 (and GrangerHub 1.3 multiprotocol servers) |
| Smokin' Guns | `master.smokin-guns.org:27950` | 68 (1.1, 1.2dev) |
| Counter-Strike 1.6 | `ms.cs16.net:27010` (filter `\gamedir\cstrike`) | GoldSrc (A2S_INFO `I` and `m`), no fixed version list |
| Battlefield 1942 | `master.openspy.net:28900` (gamename `bfield1942`) | v1.61, v1.612 |
| Quake II | `master.quakeservers.net:27900` | no fixed version list (gamename `baseq2` and mods) |
| D-Day: Normandy | `master.quakeservers.net:27900` (servers with `gamedir` `dday`) | the mod's `dll_version` (5.067…) |
| Unreal Tournament 2004 | `utmaster.openspy.net:28902`, `ut2004master.333networks.com:28902` | no fixed version list (`ServerVersion` rule: 3369, 3374…) |

CoD, CoD2 and UT were verified on 2026-10-18; the rest (including Jedi Academy, SoF2, CS 1.6 and BF1942) on 2026-10-19, by probing each master per protocol with at most 30 status queries per game (`getstatus`, `\status\`, A2S). Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns were probed on 2026-10-24 (10 `getstatus` queries per game). D-Day: Normandy was probed on 2026-10-25: the Quake II master lists every Quake II server and JASE keeps the ones whose `gamedir` is `dday` (1 of 95 that answered), which Quake II no longer shows; it is launched as `quake2.exe +set game dday +connect ip:port`. dpmaster mixes games, so JASE asks it for the game's gamename (`getservers Xonotic 3 full empty`).

The Steam games (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) have no master: they are listed through the Steam Web API (`api.steampowered.com`, needs a key; probed 2026-10-20), which the Masters tab shows as their source.

## Technical notes

### Medal of Honor and OpenMoHAA

Set the game folder: the one containing `MOHAA.exe` (Spearhead: `moh_spearhead.exe`; Breakthrough: `moh_breakthrough.exe`) or, with OpenMoHAA, the one containing its launchers. If that folder has the variant's OpenMoHAA launcher (`launch_openmohaa_base.exe` for AA, `launch_openmohaa_spearhead.exe` for Spearhead and `launch_openmohaa_breakthrough.exe` for Breakthrough), JASE uses it even if the folder still has `MOHAA.exe`: choosing an OpenMoHAA folder means choosing OpenMoHAA. Without a launcher the original exe is used; if that is missing too, AA falls back to `openmohaa.exe`. Settings shows the chosen executable (e.g. "✓ launch_openmohaa_base.exe"). The launchers pass the arguments on to `openmohaa.exe`, adding `+set com_target_game N`.

MOHAA is launched like the Quake 3 games: `+password <password> +connect ip:port`. The engine registers the `connect` command and the `password` cvar, and `+…` arguments run at startup (`Com_AddStartupCommands`; see `code/client/cl_main.cpp` and `code/Launcher/launch_main.cpp` in github.com/openmoh/openmohaa).

Probe notes (2026-10-19, 18 `\status\` queries per variant):

- The master lists the query port (12300…); `\status\` carries `hostport` with the game port, normally query port − 97 (12203), which is used as a fallback when missing.
- The three masters mix variants (e.g. AA servers in the Spearhead list), so servers whose `gamename` does not match are discarded.
- MOHAA does not advertise a password in `\status\`: no padlock is shown and JASE does not ask for one. On a password-protected server, type it in the game console (`password <password>`, then `reconnect`).
- GameTracker images: folders `mohaa`, `sh` and `bt`, with the `/` in the map name replaced by `_` (`obj/obj_team2` → `obj_obj_team2`). There is no local image extraction for MOHAA.

### Unreal Tournament 2004

The UT2004 masters (probed 2026-10-20, ~600 servers each) speak Epic's TCP protocol, as implemented by OpenSpy's `utmaster` service (github.com/openspy/openspy-core). The client answers the challenge with a fixed, fictitious CD key hash (all zeros); it never uses or derives a real CD key, and if a master rejects it JASE does not try anything else. Servers are queried over UDP on the query port (game port + 1): info, rules and players. The password padlock comes from the `GamePassword` rule and the Version column from `ServerVersion`. It is launched as `UT2004.exe ip:port?password=<password>`. Map images come from GameTracker (folder `ut2k4`); there is no local extraction for `.ut2` maps.

### Zandronum (Doom)

The Zandronum master (probed 2026-10-27, ~780 servers) and its servers speak the Skulltag/Zandronum launcher protocol, compressed with its fixed Huffman tree (ported from Zandronum's `huffman.cpp`, MIT, Timothy Landers 2009). JASE shows name, map, game mode, players (bots apart), password and the IWAD as the *mod* in the game tree. It is launched as `zandronum.exe -iwad <IWAD> -connect ip:port` (with `+cl_password`/`+cl_joinpassword` when there is a password). The IWAD and the server's PWADs must already be in Zandronum's folder: JASE does not download them (Doomseeker does, with Wadseeker).

### Version column

In Quake III, RtCW and ET the Version column shows the abbreviated `version` cvar (`ioq3 1.36+u2022…/Debian linux…` → `ioq3 1.36`); in the Call of Duty games it shows `shortversion`.

### Discarded in probing

These do not return servers or do not suit the standard client:

- Quake III: protocol 67 (1.31, 2–4 servers) and 71 (new ioq3, almost all OpenArena).
- RtCW: protocols 50 and 59 (15 servers each, almost none answered).
- ET: protocol 83 (the master only returns an empty 0.0.0.0:0 entry) and 82 ("ET 3.00" servers, 14); `master.etlegacy.com` does not answer 82/83.
- CoD4: protocols 2, 3 and 4 (no servers).
- **Counter-Strike 1.6** (Valve protocol, probed 2026-10-19): it works through `ms.cs16.net:27010` with the `\gamedir\cstrike` filter (`\appid\10` returns 0 on that master); A2S_INFO confirms the game folder. Valve's masters `hl1master.steampowered.com` and `hl2master.steampowered.com` still have no DNS (CNAMEs to `*.discovery.steamserver.net`, NXDOMAIN on Google and Cloudflare), the old IPs (`208.64.200.39/52/65:27011`) and `master.hlsw.org:27010/27011` do not answer, and `master.csmetahook.com` ignores filters and stops at page 2. It is launched as `hl.exe -game cstrike +password <password> +connect ip:port`.
- **Jedi Academy:** `masterjk3.ravensoft.com` resolves but does not answer and `master.jk3.fun` has no DNS; protocol 25 (1.00) has only 2 servers.
- **SoF2:** `sof2master.ravensoft.com`, `master.sof2.org` and `master.sof2.net` have no DNS; protocol 2003 has no servers.
- **Jedi Outcast** (2026-10-24): `masterjk2.ravensoft.com:28060` resolves but does not answer.
- **Xonotic, Warsow, Warfork** (2026-10-24): `ghdigital.com:27950` does not answer; `dpmaster.tchr.no` returns no Warfork servers.
- **World of Padman** (2026-10-24): `master.worldofpadman.net:27955` lists a single server (protocol 71) and dpmaster none (gamename `WorldofPadman`), so it was left out.
- **Tremulous:** protocols 70 and 71 have no servers on `master.tremulous.net`.
- **Battlefield 1942:** `master.bf1942.org` returns an encrypted list (enctype other than 0) and `master.333networks.com` an empty one.
- **QuakeWorld** (2026-10-27): the UDP masters (`master.quakeworld.nu`, `qwmaster.fodquake.net`, `master.quakeservers.net` and others, port 27000) do not answer and `hubapi.quakeworld.nu` could not be reached.
- **Halo: Custom Edition** (2026-10-27): `s1.master.hosthpc.com:28910` speaks GameSpy 2 with the enctypeX cipher, whose only known implementation (gslist) is GPL, not compatible with JASE's MIT license; the classic GameSpy masters (333networks, OpenSpy) list no `halom` servers.

### Map images

- **CoD2:** read from the installed game's `.iwd` files (verified). The other CoD games try the same, with no guarantee. Quake 3, Open Arena, RtCW, Enemy Territory, Urban Terror, Jedi Outcast, Jedi Academy, Soldier of Fortune II, Warsow, Warfork, Tremulous and Smokin' Guns read `levelshots/` from the `.pk3` files of their game folder, with no guarantee. Jedi Outcast, Jedi Academy, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns have no images on GameTracker (probed 2026-10-24); Xonotic shows no map image.
- **UT99:** generated from the `.unr` files in the game's `Maps` folder.
- **GameTracker (optional, off by default):** used by CoD, CoD United Offensive, CoD4, Quake 3, Open Arena, RtCW, Enemy Territory, Urban Terror, SoF2, UT99, UT2004, MOHAA and its expansions, Counter-Strike 1.6, Counter-Strike: Source, Team Fortress 2 and Classic, Left 4 Dead 1 and 2, Half-Life Deathmatch, Day of Defeat, Garry's Mod, Battlefield 1942, Quake II and D-Day (its `dday1`…`dday5` maps are in GameTracker's Quake II folder). RtCW has partial coverage: its folder is Enemy Territory's and only holds the maps both games share. Jedi Academy uses local files only. In **File → Settings** you can enable downloading images from `image.gametracker.com` for maps that are not installed. They are stored in `%APPDATA%\JimmysASE\cache\maps`. Without this option JASE downloads no images.

### Server country

The **Country** column shows each server's flag (name in the tooltip and the card), and the text filter accepts the country code (`cl`, `br`…). The country is looked up offline from the server's IPv4 in DB-IP's "IP to Country Lite" database. The first time JASE runs (and while the file is missing) it is downloaded once in the background to `%APPDATA%\JimmysASE\cache\dbip-country-lite.csv` (~4.5 MB compressed, ~30 MB on disk). Without a network the column stays empty and only the status bar says so. **Settings → Update country database** downloads it again (DB-IP publishes it monthly).

### Configuration and passwords

- Configuration (game paths, theme, version filter, quick filters, saved filters, favorites and their notes, friends, tray option, window and masters) is stored in `%APPDATA%\JimmysASE\config.json`. You can edit it by hand with the app closed; for example, the `masters` key replaces a game's masters (`{"masters": {"ut99": [["master.333networks.com", 28900]]}}`).
- Server passwords are **not** saved in that file: they go to the **Windows Credential Manager** (service `JimmysAllSeeingEye`). You can view or delete them in *Control Panel → Credential Manager → Windows Credentials*.
- Cache in `%APPDATA%\JimmysASE\cache\`: map images (`maps\`), the country database (`dbip-country-lite.csv`) and the occupancy history (`history.json`).

## Install and run from source

Requirements: Windows 10/11 and **Python 3.11 or newer for Windows** (from [python.org](https://www.python.org/downloads/windows/); tick "Add python.exe to PATH" and keep the `py` launcher). Python dependencies (installed by `pip install -e .`): PySide6, keyring and Pillow (reads map images from the game files).

From the project folder, in PowerShell:

```powershell
py -m venv .venv
.venv\Scripts\pip install -e .
.venv\Scripts\python -m jase
```

(The `.venv\Scripts\jase` command is also available.) On Linux/WSL it runs for development, but it does not launch games.

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

## Transparency

I build JASE with [Claude Code](https://claude.com/claude-code), Anthropic's AI coding assistant: much of the code, tests and documentation is written with its help, and I review, try out and decide what goes in. I maintain this project out of nostalgia for the old games and the servers we used to play on.

## License

MIT; see [LICENSE](LICENSE).

## Credits

- IP to Country Lite by [DB-IP](https://db-ip.com), CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
- Flags: [flag-icons](https://github.com/lipis/flag-icons) 7.5.0 by Panayiotis Lipiridis, MIT license (`src/jase/assets/flags/LICENSE`).
- Built with PySide6 (Qt for Python, LGPL-3.0), keyring and Pillow.
- Community masters and projects that keep these games alive: 333networks, OpenSpy, jkhub, 1fxmod, dpmaster and cs16.net.
