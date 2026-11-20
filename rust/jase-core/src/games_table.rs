// Tabla de juegos (antes generada desde games.py de 1.3.0; se edita a mano).
use super::{DEFAULT, Family, GameDef, LaunchStyle, ListFilter};

pub static GAMES: &[GameDef] = &[
    GameDef {
        id: "cod2",
        name: "Call of Duty 2",
        family: Family::Quake3,
        // master.cod2x.me: el de CoD2x (sv_master2 forzado por el parche; sondeo 2026-10-29:
        // 480 servidores, 14 que el de Activision no lista).
        masters: &[
            ("cod2master.activision.com", 20710),
            ("master.cod2x.me", 20710),
        ],
        // 1.0, 1.2, 1.3 (verificado 2026-10-18); 119/120 = CoD2x 1.4.x (2026-10-19)
        protocols: &[115, 117, 118, 119, 120],
        known_versions: &["1.0", "1.2", "1.3", "1.4"],
        default_version: "1.3",
        exe_relpath: "CoD2MP_s.exe",
        gametracker_id: "cod2",
        cod_loadscreens: true,
        version_aliases: &[("1.4", "1.4", "1.4 (CoD2x)")],
        default_path: "D:\\Juegos\\CoD2",
        mod_key: "fs_game",
        badge: "C2",
        ..DEFAULT
    },
    GameDef {
        id: "cod1",
        name: "Call of Duty",
        family: Family::Quake3,
        masters: &[("codmaster.activision.com", 20510)],
        protocols: &[1, 2, 4, 5, 6], // 1.1, 1.2, 1.3, 1.4, 1.5 (verificado 2026-10-18)
        known_versions: &["1.1", "1.2", "1.3", "1.4", "1.5"],
        default_version: "1.5",
        exe_relpath: "CoDMP.exe",
        gametracker_id: "cod",
        cod_loadscreens: true,
        mod_key: "fs_game",
        badge: "C1",
        ..DEFAULT
    },
    // --- Familia Quake 3 (CoD:UO y otros): masters sondeados el 2026-10-19. ---
    GameDef {
        id: "coduo",
        name: "Call of Duty: United Offensive",
        family: Family::Quake3,
        masters: &[("coduomaster.activision.com", 20610)],
        // 21 = 1.41, 22 = 1.51; 24 también anuncia shortversion 1.51 (servidores
        // "Activision TDM 24/7").
        protocols: &[21, 22, 24],
        known_versions: &["1.41", "1.51"],
        default_version: "1.51",
        exe_relpath: "CoDUOMP.exe",
        gametracker_id: "uo",
        cod_loadscreens: true,
        short_name: "CoD: United Offensive",
        mod_key: "fs_game",
        badge: "UO",
        ..DEFAULT
    },
    GameDef {
        id: "cod4",
        name: "Call of Duty 4: Modern Warfare",
        family: Family::Quake3,
        masters: &[("cod4master.activision.com", 20810)],
        // 6 = 1.7 (la gran mayoría, casi todo CoD4X con shortversion "-"), 7 = 1.8,
        // 5 = 1.6, 1 y 8 = servidores que anuncian 1.0/1.xu.
        protocols: &[1, 5, 6, 7, 8],
        known_versions: &["1.0", "1.6", "1.7", "1.8"],
        default_version: "Todas",
        exe_relpath: "iw3mp.exe",
        gametracker_id: "cod4",
        cod_loadscreens: true,
        short_name: "CoD4",
        mod_key: "fs_game",
        badge: "C4",
        ..DEFAULT
    },
    GameDef {
        id: "q3",
        name: "Quake III Arena",
        family: Family::Quake3,
        // dpmaster.deathmask.net: 12 servidores que no listan los demás (sondeo 2026-11-05).
        masters: &[
            ("master.ioquake3.org", 27950),
            ("master.quake3arena.com", 27950),
            ("master.maverickservers.com", 27950),
            ("dpmaster.deathmask.net", 27950),
        ],
        protocols: &[68], // 1.32 (quake3.exe); 67 = 1.31 fuera; 71 es de OpenArena
        known_versions: &["Q3 1.32e", "ioq3 1.36", "CNQ3 1.53"],
        default_version: "Todas",
        exe_relpath: "quake3.exe",
        gametracker_id: "q3",
        levelshot_dir: "baseq3",
        version_key: "version", // normalizado con quake3.short_engine_version
        mod_key: "gamename",
        base_mods: &["baseq3"],
        badge: "Q3",
        ..DEFAULT
    },
    GameDef {
        id: "rtcw",
        name: "Return to Castle Wolfenstein",
        family: Family::Quake3,
        masters: &[("wolfmaster.idsoftware.com", 27950)],
        protocols: &[57, 60], // 60 = 1.41 (WolfMP.exe); 57 = 1.0/1.33 y mods
        known_versions: &[
            "Wolf 1.0",
            "Wolf 1.41",
            "Wolf 1.41b",
            "iortcw 1.51c",
            "iortcw 1.51d",
        ],
        default_version: "Todas",
        exe_relpath: "WolfMP.exe",
        // GameTracker sondeado 2026-10-24: carpeta de ET, solo trae los mapas
        // que comparten ambos juegos.
        gametracker_id: "et",
        levelshot_dir: "main",
        version_key: "version",
        short_name: "RtCW",
        mod_key: "gamename",
        base_mods: &["main"],
        badge: "RW",
        ..DEFAULT
    },
    GameDef {
        id: "et",
        name: "Wolfenstein: Enemy Territory",
        family: Family::Quake3,
        masters: &[
            ("etmaster.idsoftware.com", 27950),
            ("master.etlegacy.com", 27950),
        ],
        protocols: &[84], // 2.60b y ET: Legacy
        known_versions: &["ET Legacy v2.83.2", "ET Legacy v2.86.2"],
        default_version: "Todas",
        exe_relpath: "ET.exe",
        // ET: Legacy (etl.exe) es el cliente actual; si convive con ET.exe, manda.
        exe_candidates: &["etl.exe"],
        preferred_exes: &["etl.exe"],
        gametracker_id: "et",
        levelshot_dir: "etmain",
        version_key: "version",
        short_name: "Wolfenstein: ET",
        mod_key: "gamename",
        base_mods: &["etmain"],
        badge: "ET",
        ..DEFAULT
    },
    // --- ioquake3 (sondeo del 2026-10-20): el cvar `version` varía mucho por mod/cliente. ---
    GameDef {
        id: "urt",
        name: "Urban Terror",
        family: Family::Quake3,
        masters: &[("master.urbanterror.info", 27900)],
        protocols: &[68],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "Quake3-UrT.exe",
        exe_candidates: &["Quake3-UrT-x86_64.exe"],
        gametracker_id: "urbanterror",
        levelshot_dir: "q3ut4",
        version_key: "version",
        mod_key: "gamename",
        base_mods: &["q3urt43"], // gamename de los servidores 4.3.x en el sondeo
        badge: "UR",             // "UT" ya es de UT99
        ..DEFAULT
    },
    GameDef {
        id: "oa",
        name: "OpenArena",
        family: Family::Quake3,
        masters: &[
            ("dpmaster.deathmask.net", 27950),
            ("master.ioquake3.org", 27950),
        ],
        protocols: &[71],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "openarena.exe",
        exe_candidates: &["openarena.x86_64.exe"],
        gametracker_id: "q3", // GameTracker sondeado 2026-10-24
        levelshot_dir: "baseoa",
        // Sondeo del 2026-10-20 (113 servidores del protocolo 71 en ambos masters): 14 traen
        // gamename=baseq3 ("Q3 1.32e", Quake III) y 2 defrag (mod de Q3); el resto es
        // baseoa o un mod de OA (excessiveplus), por eso se excluye y no se exige baseoa.
        status_exclude_mods: &["baseq3", "defrag"],
        version_key: "version",
        mod_key: "gamename",
        base_mods: &["baseoa"],
        badge: "OA",
        ..DEFAULT
    },
    // --- Raven (Quake 3): masters sondeados el 2026-10-19. ---
    // JKA no envía fs_game en \getstatus: el mod solo viene en `gamename`.
    GameDef {
        id: "jka",
        name: "Star Wars Jedi Knight: Jedi Academy",
        family: Family::Quake3,
        masters: &[("master.jkhub.org", 29060)],
        protocols: &[26], // 26 = 1.01; 25 (1.00) casi no tiene servidores
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "GameData\\jamp.exe",
        exe_candidates: &["jamp.exe"],
        // OpenJK no lleva el DRM de Steam; si está junto a jamp.exe, se usa.
        preferred_exes: &["openjk.x86_64.exe", "openjk.x86.exe"],
        levelshot_dir: "base",
        version_key: "version",
        steam_appid: 6020,
        short_name: "Jedi Academy",
        mod_key: "gamename",
        base_mods: &["basejka"],
        badge: "JK",
        ..DEFAULT
    },
    // El cvar `version` varía (SOF2MP V1.02a, SoF2Plus 1.00_g…): la versión sale del
    // protocolo (2002 = 1.00, 2004 = 1.03), ver quake3.PROTOCOL_VERSIONS.
    GameDef {
        id: "sof2",
        name: "Soldier of Fortune II",
        family: Family::Quake3,
        masters: &[("master.1fxmod.org", 20110)],
        protocols: &[2002, 2004],
        known_versions: &["1.00", "1.03"],
        default_version: "Todas",
        exe_relpath: "SoF2MP.exe",
        gametracker_id: "sof2",
        levelshot_dir: "base",
        version_key: "protocol",
        short_name: "SoF2",
        mod_key: "fs_game",
        badge: "S2",
        ..DEFAULT
    },
    // --- Sondeo del 2026-10-24 (JASE 0.12). ---
    GameDef {
        id: "jk2",
        name: "Star Wars Jedi Knight II: Jedi Outcast",
        family: Family::Quake3,
        // masterjk2.ravensoft.com:28060 no responde (sondeo 2026-10-24); 15 = 1.02,
        // 16 = 1.04 (casi todo JK2MV, que anuncia "JK2MP: v1.04mv").
        // master.ouned.de:28060 suma 2 que jkhub no lista (sondeo 2026-11-05).
        masters: &[("master.jkhub.org", 28060), ("master.ouned.de", 28060)],
        protocols: &[15, 16],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "GameData\\jk2mp.exe",
        exe_candidates: &["jk2mp.exe"],
        preferred_exes: &["jk2mvmp.exe"], // cliente de JK2MV, sin el DRM de Steam
        levelshot_dir: "base",
        version_key: "version",
        steam_appid: 6030,
        short_name: "Jedi Outcast",
        mod_key: "gamename",
        base_mods: &["basejk"],
        badge: "J2",
        ..DEFAULT
    },
    // dpmaster mezcla juegos: la consulta lleva `master_gamename`; ghdigital.com:27950 no
    // responde (sondeo 2026-10-24).
    GameDef {
        id: "xonotic",
        name: "Xonotic",
        family: Family::Quake3,
        masters: &[
            ("dpmaster.deathmask.net", 27950),
            ("dpmaster.tchr.no", 27950),
        ],
        protocols: &[3],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "xonotic.exe",
        exe_candidates: &["xonotic-x86_64.exe"],
        master_gamename: "Xonotic",
        // DarkPlaces no manda `version`; `gameversion` es el número de versión (806 = 0.8.6).
        version_key: "gameversion",
        mod_key: "modname",
        base_mods: &["data"],
        badge: "XO",
        ..DEFAULT
    },
    GameDef {
        id: "warsow",
        name: "Warsow",
        family: Family::Quake3,
        masters: &[
            ("dpmaster.deathmask.net", 27950),
            ("dpmaster.tchr.no", 27950),
        ],
        protocols: &[22],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "warsow_x64.exe",
        exe_candidates: &["warsow_x86.exe"],
        levelshot_dir: "basewsw",
        master_gamename: "Warsow",
        version_key: "version",
        mod_key: "fs_game",
        base_mods: &["basewsw"],
        badge: "WS",
        ..DEFAULT
    },
    GameDef {
        id: "warfork",
        name: "Warfork",
        family: Family::Quake3,
        // dpmaster.tchr.no devuelve 0 servidores de Warfork (sondeo 2026-10-24).
        masters: &[("dpmaster.deathmask.net", 27950)],
        protocols: &[26],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "warfork_x64.exe",
        levelshot_dir: "basewf",
        master_gamename: "Warfork",
        version_key: "version",
        mod_key: "fs_game",
        base_mods: &["basewf"],
        badge: "WF",
        ..DEFAULT
    },
    GameDef {
        id: "tremulous",
        name: "Tremulous",
        family: Family::Quake3,
        masters: &[("master.tremulous.net", 30710)],
        protocols: &[69],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "tremulous.exe",
        levelshot_dir: "base",
        version_key: "version",
        badge: "TR",
        ..DEFAULT
    },
    GameDef {
        id: "smokinguns",
        name: "Smokin' Guns",
        family: Family::Quake3,
        masters: &[("master.smokin-guns.org", 27950)],
        protocols: &[68],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "smokinguns.exe",
        levelshot_dir: "smokinguns",
        version_key: "version",
        badge: "SG",
        ..DEFAULT
    },
    GameDef {
        id: "ut99",
        name: "Unreal Tournament",
        family: Family::Gamespy,
        masters: &[
            ("master.333networks.com", 28900),
            ("master.openspy.net", 28900),
        ],
        protocols: &[],
        known_versions: &["436", "451", "469"],
        default_version: "Todas",
        exe_relpath: "System\\UnrealTournament.exe",
        gametracker_id: "ut", // GameTracker sondeado 2026-10-24
        gamespy_gamename: "ut",
        gamespy_key: "Z5Nfb0",
        launch_style: LaunchStyle::Unreal,
        connect_port_fallback: -1,
        default_path: "D:\\Juegos\\UnrealTournament",
        badge: "UT",
        list_filter: ListFilter::NotTacops,
        ..DEFAULT
    },
    // Sin master propio: va en la lista `ut` (sondeo 2026-10-30: 59 de 791 servidores) y
    // solo el modo lo distingue. `gamever` es el motor (469): la versión sale del gametype.
    // GameTracker no tiene la carpeta (404 en to, tacops, tacticalops y tactical).
    GameDef {
        id: "tacops",
        name: "Tactical Ops",
        family: Family::Gamespy,
        masters: &[
            ("master.333networks.com", 28900),
            ("master.openspy.net", 28900),
        ],
        protocols: &[],
        known_versions: &["3.4", "3.5"],
        default_version: "Todas",
        exe_relpath: "System\\TacticalOps.exe",
        gamespy_gamename: "ut",
        gamespy_key: "Z5Nfb0",
        launch_style: LaunchStyle::Unreal,
        connect_port_fallback: -1,
        version_key: "gametype",
        version_aliases: &[("TO340", "3.4", "3.4"), ("TO350", "3.5", "3.5")],
        badge: "TO",
        list_filter: ListFilter::Tacops,
        version_dirs: &[("3.4", "TO340"), ("3.5", "TO350")],
        ..DEFAULT
    },
    // --- Medal of Honor (GameSpy): masters sondeados el 2026-10-19. ---
    // El master lista el puerto de consulta (12300); el \status\ trae `hostport`
    // (normalmente consulta − 97, 12203). MOHAA no envía `password` en \status\.
    GameDef {
        id: "mohaa",
        name: "Medal of Honor: Allied Assault",
        family: Family::Gamespy,
        masters: &[
            ("master.333networks.com", 28900),
            ("master.openspy.net", 28900),
        ],
        protocols: &[],
        known_versions: &["1.11"], // OpenMoHAA anuncia "1.12+0.8x.y" (se añade al verlo)
        default_version: "Todas",
        exe_relpath: "MOHAA.exe",
        exe_candidates: &["launch_openmohaa_base.exe", "openmohaa.exe"],
        preferred_exes: &["launch_openmohaa_base.exe"],
        gametracker_id: "mohaa",
        gamespy_gamename: "mohaa",
        gamespy_key: "M5Fdwc",
        status_gamename: "mohaa",
        connect_port_fallback: -97,
        default_path: "D:\\Juegos\\MOHAA_STOCK",
        short_name: "MOHAA: Allied Assault",
        badge: "MA",
        ..DEFAULT
    },
    GameDef {
        id: "mohaas",
        name: "Medal of Honor: Spearhead",
        family: Family::Gamespy,
        masters: &[
            ("master.333networks.com", 28900),
            ("master.openspy.net", 28900),
        ],
        protocols: &[],
        known_versions: &["2.15"],
        default_version: "Todas",
        exe_relpath: "moh_spearhead.exe",
        // Solo el lanzador: openmohaa.exe sin "+set com_target_game 1" arrancaría AA.
        exe_candidates: &["launch_openmohaa_spearhead.exe"],
        preferred_exes: &["launch_openmohaa_spearhead.exe"],
        gametracker_id: "sh",
        gamespy_gamename: "mohaas",
        gamespy_key: "h2P1c9",
        status_gamename: "mohaas",
        connect_port_fallback: -97,
        short_name: "MOHAA: Spearhead",
        badge: "SH",
        ..DEFAULT
    },
    GameDef {
        id: "mohaab",
        name: "Medal of Honor: Breakthrough",
        family: Family::Gamespy,
        masters: &[
            ("master.333networks.com", 28900),
            ("master.openspy.net", 28900),
        ],
        protocols: &[],
        known_versions: &["2.40"],
        default_version: "Todas",
        exe_relpath: "moh_breakthrough.exe",
        exe_candidates: &["launch_openmohaa_breakthrough.exe"],
        preferred_exes: &["launch_openmohaa_breakthrough.exe"],
        gametracker_id: "bt",
        gamespy_gamename: "mohaab",
        gamespy_key: "y32FDc",
        status_gamename: "mohaab",
        connect_port_fallback: -97,
        short_name: "MOHAA: Breakthrough",
        badge: "BT",
        ..DEFAULT
    },
    // --- Counter-Strike 1.6 (Valve): master sondeado el 2026-10-19. ---
    // El master mezcla juegos de GoldSrc; el filtro \\gamedir\\cstrike lo acota y
    // el A2S_INFO confirma la carpeta. Las versiones varían (HLDS 0x49 trae
    // "1.1.2.7/Stdio", 0x6D no trae ninguna), así que no hay lista fija.
    // ms.cs16.net es no-steam y casi todo europeo (sondeo 2026-10-29: 231 servidores, ninguno
    // de Latinoamérica); los servidores con Steam (incluidos los ReHLDS) solo están en la API
    // web de Steam (los masters UDP de Valve ya no resuelven): se suma si hay clave.
    GameDef {
        id: "cs16",
        name: "Counter-Strike 1.6",
        family: Family::Valve,
        masters: &[("ms.cs16.net", 27010), ("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "hl.exe",
        gametracker_id: "cs",
        launch_style: LaunchStyle::Valve,
        valve_appid: 10,
        valve_gamedir: "cstrike",
        valve_master_filter: "\\gamedir\\cstrike",
        steam_webapi_optional: true,
        short_name: "CS 1.6",
        badge: "CS",
        ..DEFAULT
    },
    // --- Juegos de Valve listados por la API web de Steam (necesitan clave de API). ---
    // Los de Source se unen con steam://connect (sin ruta); los de GoldSrc, con hl.exe.
    // Sondeo real 2026-10-20 (muestra de 10 por juego, A2S): versiones vistas L4D 1.0.4.5,
    // L4D2 2.2.4.3, CS:S 11003710, TF2 11138802, HL DM 1.1.2.7, DoD 1.1.2.6, TFC 1.1.2.2.
    // Listados: L4D 57, L4D2 1890, CS:S 10000 (tope de la API), TF2 270, HL DM 46, DoD 9, TFC 8.
    GameDef {
        id: "l4d",
        name: "Left 4 Dead",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "left4dead.exe",
        gametracker_id: "l4d", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Steam,
        valve_appid: 500,
        valve_gamedir: "left4dead",
        steam_webapi: true,
        short_name: "L4D",
        badge: "L4",
        ..DEFAULT
    },
    GameDef {
        id: "l4d2",
        name: "Left 4 Dead 2",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "left4dead2.exe",
        gametracker_id: "left4dead2", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Steam,
        valve_appid: 550,
        valve_gamedir: "left4dead2",
        steam_webapi: true,
        short_name: "L4D2",
        badge: "L2",
        ..DEFAULT
    },
    GameDef {
        id: "css",
        name: "Counter-Strike: Source",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "hl2.exe",
        gametracker_id: "css", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Steam,
        valve_appid: 240,
        valve_gamedir: "cstrike",
        steam_webapi: true,
        short_name: "CS: Source",
        badge: "SS",
        ..DEFAULT
    },
    GameDef {
        id: "tf2",
        name: "Team Fortress 2",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "tf_win64.exe",
        exe_candidates: &["hl2.exe"],
        gametracker_id: "tf2", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Steam,
        valve_appid: 440,
        valve_gamedir: "tf",
        steam_webapi: true,
        short_name: "TF2",
        badge: "TF",
        ..DEFAULT
    },
    GameDef {
        id: "hldm",
        name: "Half-Life Deathmatch",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "hl.exe",
        gametracker_id: "hl", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Valve,
        valve_appid: 70,
        valve_gamedir: "valve",
        valve_master_filter: "\\appid\\70\\gamedir\\valve",
        steam_webapi: true,
        short_name: "HL DM",
        badge: "HL",
        ..DEFAULT
    },
    GameDef {
        id: "dod",
        name: "Day of Defeat",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "hl.exe",
        gametracker_id: "dod", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Valve,
        valve_appid: 30,
        valve_gamedir: "dod",
        steam_webapi: true,
        short_name: "DoD 1.3",
        badge: "DD",
        ..DEFAULT
    },
    GameDef {
        id: "tfc",
        name: "Team Fortress Classic",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "hl.exe",
        gametracker_id: "tfc", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Valve,
        valve_appid: 20,
        valve_gamedir: "tfc",
        valve_master_filter: "\\appid\\20\\gamedir\\tfc",
        steam_webapi: true,
        short_name: "TFC",
        badge: "TC",
        ..DEFAULT
    },
    // Sondeo 2026-10-25: 1064 servidores no vacíos en la API web, versión 2026.04.29 (A2S);
    // GameTracker: carpeta `garrysmod` (gm_construct, gm_flatgrass).
    GameDef {
        id: "gmod",
        name: "Garry's Mod",
        family: Family::Valve,
        masters: &[("api.steampowered.com", 443)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        // Rama de 32 bits (hl2.exe) o de 64 bits (bin\win64\gmod.exe); la carpeta
        // garrysmod\ la distingue de CS: Source.
        exe_relpath: "hl2.exe",
        exe_candidates: &["bin\\win64\\gmod.exe"],
        gametracker_id: "garrysmod",
        launch_style: LaunchStyle::Steam,
        valve_appid: 4000,
        valve_gamedir: "garrysmod",
        steam_webapi: true,
        badge: "GM",
        ..DEFAULT
    },
    // --- Battlefield 1942 (GameSpy): master sondeado el 2026-10-19. ---
    // El master lista el puerto de consulta (23000+); el juego va en `hostport` (14567).
    GameDef {
        id: "bf1942",
        name: "Battlefield 1942",
        family: Family::Gamespy,
        masters: &[("master.openspy.net", 28900)],
        protocols: &[],
        known_versions: &["v1.61", "v1.612"],
        default_version: "Todas",
        exe_relpath: "BF1942.exe",
        gametracker_id: "bf1942",
        gamespy_gamename: "bfield1942",
        gamespy_key: "HpWx9z",
        status_gamename: "bfield1942",
        launch_style: LaunchStyle::Bf1942,
        // hostport - puerto de consulta fue -8433 en 2 de 3 capturas.
        connect_port_fallback: -8433,
        short_name: "BF1942",
        // Los servidores anuncian el mod (carpeta) en `gameId`; "bf1942" es el juego base.
        mod_key: "gameId",
        base_mods: &["bf1942"],
        badge: "BF",
        ..DEFAULT
    },
    // --- Quake II (familia quake2): master sondeado el 2026-10-20. ---
    GameDef {
        id: "q2",
        name: "Quake II",
        family: Family::Quake2,
        masters: &[("master.quakeservers.net", 27900)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "quake2.exe",
        exe_candidates: &["q2pro.exe", "yquake2.exe"],
        gametracker_id: "q2",           // GameTracker sondeado 2026-10-24
        status_exclude_mods: &["dday"], // D-Day va como juego aparte
        version_key: "version",
        mod_key: "gamename",
        base_mods: &["baseq2"],
        badge: "Q2",
        ..DEFAULT
    },
    // D-Day: Normandy, mod de Quake II con lista propia: el mismo master, filtrado por la
    // `gamedir` (sondeo 2026-10-25: 1 de 95 servidores, «D-Day Chile»). La versión del mod
    // va en `dll_version` (`version` es la del motor, p. ej. q2proded).
    GameDef {
        id: "dday",
        name: "D-Day: Normandy",
        family: Family::Quake2,
        masters: &[("master.quakeservers.net", 27900)],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "quake2.exe",
        exe_candidates: &["q2pro.exe", "r1q2.exe", "yquake2.exe"],
        gametracker_id: "q2", // dday1…dday5 están en la carpeta de Quake II (2026-10-25)
        status_gamedir: "dday",
        version_key: "dll_version",
        short_name: "D-Day",
        badge: "DY",
        ..DEFAULT
    },
    // --- Unreal Tournament 2004 (familia ut2004): masters sondeados el 2026-10-20. ---
    // El master da IP, puerto de juego y de consulta (juego + 1); se consulta por UDP.
    // GameTracker no tiene imágenes de UT2004 (ut2k4/ut2004 dan 404).
    GameDef {
        id: "ut2004",
        name: "Unreal Tournament 2004",
        family: Family::Ut2004,
        masters: &[
            ("utmaster.openspy.net", 28902),
            ("ut2004master.333networks.com", 28902),
        ],
        protocols: &[],
        known_versions: &[],
        default_version: "Todas",
        exe_relpath: "System\\UT2004.exe",
        gametracker_id: "ut2k4", // GameTracker sondeado 2026-10-24
        launch_style: LaunchStyle::Unreal,
        short_name: "UT2004",
        badge: "U4",
        ..DEFAULT
    },
    // Doom multijugador. Master sondeado el 2026-10-27: 780 servidores, 118 de 250
    // consultados respondieron y todos se interpretaron (protocolo en protocols/zandronum).
    GameDef {
        id: "zandronum",
        name: "Zandronum (Doom)",
        family: Family::Zandronum,
        masters: &[("master.zandronum.com", 15300)],
        protocols: &[],
        known_versions: &["3.1", "3.2", "3.2.1", "3.3"],
        default_version: "Todas",
        exe_relpath: "zandronum.exe",
        launch_style: LaunchStyle::Zandronum,
        short_name: "Zandronum",
        mod_key: "iwad", // el «mod» es el IWAD: DOOM2.WAD, DOOM.WAD, freedoom2.wad...
        badge: "ZD",
        ..DEFAULT
    },
];
