# jase-cli: server queries without a GUI

`jase-cli` runs JASE's query engine (masters + UDP server queries) from the command line. It
needs only the Python standard library: `pip install jase` installs no Qt, keyring or Pillow.
The desktop app is the `gui` extra: `pip install "jase[gui]"`.

```sh
pip install "jase @ git+https://github.com/JotaJazz/JASE@<tag>"
jase-cli games --json
jase-cli scan cod2 --json --country --timeout 120
```

## Commands

`jase-cli games [--json]` lists the supported games.

`jase-cli scan GAME [options]` asks the game's masters for the server list and queries
every server.

| Option | Meaning |
|---|---|
| `--json` | Machine-readable output (contract below). Without it: a text table on stdout, errors and summary on stderr. |
| `--steam-key KEY` | Steam Web API key for games listed through Steam (`needs_steam_key`). Default: the `JASE_STEAM_KEY` environment variable. |
| `--include-empty` | Steam games: also list empty servers (by default only non-empty ones; the full list is huge). |
| `--timeout SECONDS` | Overall time limit. When reached, the output has `"timed_out": true` and the servers that had answered so far. |
| `--country` | Add each server's country (ISO code). Downloads the DB-IP Lite database (CC BY 4.0) to the cache folder the first time. |
| `--master HOST:PORT` | Query this master instead of the official ones (repeatable). |
| `--cache-dir DIR` | Cache folder (official master list, country database). Default: `JASE_CACHE_DIR`, else the desktop app's cache. |
| `--no-update` | Do not download the official master list. Otherwise it is refreshed from GitHub at most once a day; a failed download falls back to the cached or bundled list. |

Messages follow `JASE_LANG` (`es`, `en`, `pt`, `fr`, `de`, `it`), else `LC_ALL`/`LC_MESSAGES`/`LANG`,
else English. Only `message` fields and the text output are translated; keys and `kind`
values never are.

Exit codes: `0` at least one master answered (or `--timeout` was reached), `1` no master
answered, `2` usage error (unknown game, bad option).

## JSON contract, schema 1

Every document carries `"schema": 1`. Fields may be **added** within schema 1; a field
removed, renamed or changed in meaning raises the schema number.

### `games`

```json
{
 "schema": 1,
 "jase": "1.0.0",
 "games": [
  {"id": "cod2", "name": "Call of Duty 2", "short_name": "Call of Duty 2",
   "family": "quake3", "needs_steam_key": false,
   "masters": [{"host": "cod2master.activision.com", "port": 20710}]}
 ]
}
```

`family` is one of `quake3`, `quake2`, `gamespy`, `valve`, `ut2004`, `zandronum`.
`masters` is the official list as of this run.

### `scan`

```json
{
 "schema": 1,
 "jase": "1.0.0",
 "game": {"id": "bf1942", "name": "Battlefield 1942"},
 "duration_s": 5.7,
 "timed_out": false,
 "masters": [{"host": "master.333networks.com", "port": 28900, "ok": true}],
 "masters_ok": 1,
 "errors": [],
 "servers_listed": 80,
 "servers_responded": 78,
 "servers": [
  {
   "address": "51.81.48.224:23000",
   "connect": "51.81.48.224:14567",
   "connect_port_deduced": false,
   "name": "MoonGamers.com | Est. 2004",
   "map": "battle of the bulge",
   "gametype": "conquest",
   "mod": null,
   "players": 33, "max_players": 64, "bots": 0, "humans": 33,
   "password": false,
   "version": "v1.61",
   "ping_ms": 120,
   "country": "us",
   "player_list": [{"name": "Jimmy", "score": 10, "ping": 40}],
   "rules": {"gameId": "bf1942", "hostport": "14567"}
  }
 ]
}
```

- `masters[].ok`: `null` when `--timeout` cut the run before the masters answered.
- `masters_ok`, `servers_listed`: `null` when timed out.
- `errors[]`: one entry per failed master: `master` (`"host:port"`), `kind` and a translated `message`. `kind` is one of:
  - `missing_key`: Steam game without `--steam-key` / `JASE_STEAM_KEY`.
  - `invalid_key`: the Steam API rejected the key (HTTP 401/403).
  - `rate_limited`: the Steam API limited the queries (HTTP 429).
  - `unsupported_host`: a Steam master other than `api.steampowered.com` (the key is never sent).
  - `busy`: the master asks to wait between queries (Zandronum).
  - `timeout`: the master did not answer in time.
  - `network`: connection or DNS error, or an HTTP error from a web master.
  - `protocol`: the master answered something that is not a server list.
- `address`: where the server answers queries. `connect`: where the game connects. They differ for GameSpy games (BF1942, UT99, MOHAA…).
- `connect_port_deduced`: `true` when the server did not report its game port (`hostport`) and JASE derived it from the query port (BF1942: query − 8433; UT99: query − 1).
- `map`, `gametype`: as the server reports them (e.g. JKA `g_gametype` is a number, BF1942 maps are display names). `rules` has every raw key the server sent.
- `mod`: the mod folder in lower case (`pam`, `nq`, `japro`…), `null` for the base game or games without mods.
- `bots`: how many of `players` are bots; `humans` = `players` − `bots`.
- `version`: the server's version string, as shown in the app's Version column (e.g. `JK2MP: v1.04mv`, `ET Legacy v2.85.0`).
- `country`: lower-case ISO 3166 code, `null` without `--country` or when unknown.
- `servers`: in arrival order.
