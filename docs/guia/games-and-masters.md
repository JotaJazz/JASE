**English** | [Español](juegos-y-masters.md)

# Games and masters

Which master servers JASE uses for each game, how they were verified, and per-game notes. For using the app see the [user guide](user-guide.md).

## Verification status

Masters were verified on 2026-10-18/19, and the Steam Web API and the UT2004 masters on 2026-10-20; see [Masters used](#masters-used).
Half-Life Deathmatch, Day of Defeat and Team Fortress Classic are listed through the Steam Web API (probed 2026-10-20), and Garry's Mod too (probed 2026-10-25: 1064 non-empty servers).
The executables and connection arguments for Jedi Academy, SoF2, CS 1.6, BF1942, Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns (`+connect`) and D-Day (`+set game dday`) and joining Tactical Ops (`host:port?password=`) are still pending verification on Windows.

## Masters used

| Game | Master | Protocols / versions |
|---|---|---|
| Call of Duty 2 | `cod2master.activision.com:20710`, `master.cod2x.me:20710` (CoD2x's) | 115 = 1.0, 117 = 1.2, 118 = 1.3, 119/120 = CoD2x 1.4.x (joining needs the CoD2x client) |
| Call of Duty | `codmaster.activision.com:20510` | 1 = 1.1, 2 = 1.2, 4 = 1.3, 5 = 1.4, 6 = 1.5 |
| Unreal Tournament | `master.333networks.com:28900`, `master.openspy.net:28900` | 436, 451, 469 |
| Tactical Ops | `master.333networks.com:28900`, `master.openspy.net:28900` (the UT list) | 3.4, 3.5 (from `gametype`: `TO340`, `TO350`) |
| Call of Duty: United Offensive | `coduomaster.activision.com:20610` | 21 = 1.41, 22 = 1.51, 24 = 1.51 |
| Call of Duty 4 | `cod4master.activision.com:20810` | 6 = 1.7 (and CoD4X, version "-"), 7 = 1.8, 5 = 1.6, 1 and 8 = 1.0 |
| Quake III Arena | `master.ioquake3.org:27950`, `master.quake3arena.com:27950`, `master.maverickservers.com:27950`, `dpmaster.deathmask.net:27950` | 68 = 1.32 (Q3 1.32e, ioq3 1.36, CNQ3 1.53) |
| Return to Castle Wolfenstein | `wolfmaster.idsoftware.com:27950` | 57, 60 = 1.41 (Wolf 1.0, Wolf 1.41/1.41b, iortcw 1.51) |
| Wolfenstein: Enemy Territory | `etmaster.idsoftware.com:27950`, `master.etlegacy.com:27950` | 84 = 2.60b / ET: Legacy |
| Medal of Honor: Allied Assault | `master.333networks.com:28900`, `master.openspy.net:28900` (gamename `mohaa`) | 1.11 (OpenMoHAA: 1.12+0.8x) |
| Medal of Honor: Spearhead | same (gamename `mohaas`) | 2.15 (OpenMoHAA: 2.16+0.8x) |
| Medal of Honor: Breakthrough | same (gamename `mohaab`) | 2.40 (OpenMoHAA: 2.41+0.8x) |
| Jedi Academy | `master.jkhub.org:29060` | 26 = 1.01 |
| Soldier of Fortune II | `master.1fxmod.org:20110` | 2002 = 1.00, 2004 = 1.03 |
| Urban Terror | `master.urbanterror.info:27900` | 68 (ioQ3 1.35 and mods; gamename `q3urt43`) |
| OpenArena | `dpmaster.deathmask.net:27950`, `master.ioquake3.org:27950` | 71 (ioq3+oa 1.36; gamename `baseoa`) |
| Jedi Outcast | `master.jkhub.org:28060`, `master.ouned.de:28060` | 15 = 1.02, 16 = 1.04 (mostly JK2MV, `JK2MP: v1.04mv`) |
| Xonotic | `dpmaster.deathmask.net:27950`, `dpmaster.tchr.no:27950` (gamename `Xonotic`) | 3 (`gameversion` 806 = 0.8.6) |
| Warsow | `dpmaster.deathmask.net:27950`, `dpmaster.tchr.no:27950` (gamename `Warsow`) | 22 = 2.10 |
| Warfork | `dpmaster.deathmask.net:27950` (gamename `Warfork`) | 26 = 2.15 |
| Tremulous | `master.tremulous.net:30710` | 69 = 1.1 (and GrangerHub 1.3 multiprotocol servers) |
| Smokin' Guns | `master.smokin-guns.org:27950` | 68 (1.1, 1.2dev) |
| Counter-Strike 1.6 | `ms.cs16.net:27010` (filter `\gamedir\cstrike`, non-Steam) and, with a Steam Web API key, `api.steampowered.com` (`\appid\10\gamedir\cstrike`) | GoldSrc (A2S_INFO `I` and `m`), no fixed version list |
| Battlefield 1942 | `master.openspy.net:28900` (gamename `bfield1942`) | v1.61, v1.612 |
| Quake II | `master.quakeservers.net:27900` | no fixed version list (gamename `baseq2` and mods) |
| D-Day: Normandy | `master.quakeservers.net:27900` (servers with `gamedir` `dday`) | the mod's `dll_version` (5.067…) |
| Unreal Tournament 2004 | `utmaster.openspy.net:28902`, `ut2004master.333networks.com:28902` | no fixed version list (`ServerVersion` rule: 3369, 3374…) |

CoD, CoD2 and UT were verified on 2026-10-18; the rest (including Jedi Academy, SoF2, CS 1.6 and BF1942) on 2026-10-19, by probing each master per protocol with at most 30 status queries per game (`getstatus`, `\status\`, A2S). Jedi Outcast, Xonotic, Warsow, Warfork, Tremulous and Smokin' Guns were probed on 2026-10-24 (10 `getstatus` queries per game). D-Day: Normandy was probed on 2026-10-25: the Quake II master lists every Quake II server and JASE keeps the ones whose `gamedir` is `dday` (1 of 95 that answered), which Quake II no longer shows; it is launched as `quake2.exe +set game dday +connect ip:port`. dpmaster mixes games, so JASE asks it for the game's gamename (`getservers Xonotic 3 full empty`). Tactical Ops was probed on 2026-10-30: it has no master of its own and announces itself as `ut`, so JASE takes the UT list and splits it by `gametype` (`TO<number>`, `TOST…`, `s_SWATGame`) or the `tostver` rule (59 of the 791 that answered). With separate `TO340\` and `TO350\` installs, JASE opens the one matching the server; `s_SWATGame` servers open the first one.

The Steam games (L4D, L4D2, CS: Source, TF2, HL DM, DoD, TFC, Garry's Mod) have no master: they are listed through the Steam Web API (`api.steampowered.com`, needs a key; probed 2026-10-20), which the Masters tab shows as their source. The API answers at most 10,000 servers per query and some IPs publish hundreds of fake ones (CS 1.6, 2026-10-29: about 200 IPs with 200 "This is a dummy CS 1.6 server!" or copied names each), so JASE drops IPs with 100 servers or more and, while the answer is cut at the cap, asks again without them.

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
- **Counter-Strike 1.6** (Valve protocol, probed 2026-10-19): it works through `ms.cs16.net:27010` with the `\gamedir\cstrike` filter (`\appid\10` returns 0 on that master); A2S_INFO confirms the game folder. Valve's masters `hl1master.steampowered.com` and `hl2master.steampowered.com` still have no DNS (CNAMEs to `*.discovery.steamserver.net`, NXDOMAIN on Google and Cloudflare), Valve's old master IPs (port 27011) and `master.hlsw.org:27010/27011` do not answer, and `master.csmetahook.com` ignores filters and stops at page 2. It is launched as `hl.exe -game cstrike +password <password> +connect ip:port`.
- **Jedi Academy:** `masterjk3.ravensoft.com` resolves but does not answer and `master.jk3.fun` has no DNS; protocol 25 (1.00) has only 2 servers.
- **SoF2:** `sof2master.ravensoft.com`, `master.sof2.org` and `master.sof2.net` have no DNS; protocol 2003 has no servers.
- **Jedi Outcast** (2026-10-24): `masterjk2.ravensoft.com:28060` resolves but does not answer.
- **Xonotic, Warsow, Warfork** (2026-10-24): `ghdigital.com:27950` does not answer; `dpmaster.tchr.no` returns no Warfork servers.
- **World of Padman** (2026-10-24): `master.worldofpadman.net:27955` lists a single server (protocol 71) and dpmaster none (gamename `WorldofPadman`), so it was left out.
- **Tremulous:** protocols 70 and 71 have no servers on `master.tremulous.net`.
- **Battlefield 1942:** `master.bf1942.org` returns an encrypted list (enctype other than 0) and `master.333networks.com` an empty one.
- **QuakeWorld** (2026-10-27): the UDP masters (`master.quakeworld.nu`, `qwmaster.fodquake.net`, `master.quakeservers.net` and others, port 27000) do not answer and `hubapi.quakeworld.nu` could not be reached.
- **Halo: Custom Edition** (2026-10-27): `s1.master.hosthpc.com:28910` speaks GameSpy 2 with the enctypeX cipher, whose only known implementation (gslist) is GPL, not compatible with JASE's MIT license; the classic GameSpy masters (333networks, OpenSpy) list no `halom` servers.

