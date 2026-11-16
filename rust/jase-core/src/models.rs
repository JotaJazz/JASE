//! Traducción de `src/jase/models.py`.
use indexmap::IndexMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServerAddress {
    pub host: String,
    pub port: u16,
}

impl ServerAddress {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub name: String,
    pub score: i32,
    pub ping: i32,
    /// Nombre con códigos de color ^N (solo CoD); "" si no aplica.
    pub raw_name: String,
}

impl Player {
    pub fn new(name: impl Into<String>, score: i32, ping: i32) -> Self {
        Self {
            name: name.into(),
            score,
            ping,
            raw_name: String::new(),
        }
    }
}

// Como en Python (`compare=False`): raw_name no cuenta para la igualdad.
impl PartialEq for Player {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name && self.score == o.score && self.ping == o.ping
    }
}

#[derive(Clone, Debug)]
pub struct ServerInfo {
    pub address: ServerAddress,
    pub connect_address: ServerAddress,
    pub name: String,
    pub map: String,
    pub gametype: String,
    pub num_players: i32,
    pub max_players: i32,
    pub password: bool,
    pub version: String,
    pub ping_ms: u32,
    pub players: Vec<Player>,
    pub rules: IndexMap<String, String>,
    /// Nombre con códigos de color ^N (solo CoD); "" si no aplica.
    pub raw_name: String,
    /// false = favorito que no respondió.
    pub online: bool,
    /// Cuántos de num_players son bots.
    pub bots: i32,
    /// GameSpy sin `hostport`: el puerto de juego se dedujo del de consulta.
    pub connect_port_deduced: bool,
}

impl ServerInfo {
    /// Servidor vacío con esas direcciones; el parser llena el resto.
    pub fn new(address: ServerAddress, connect_address: ServerAddress) -> Self {
        Self {
            address,
            connect_address,
            name: String::new(),
            map: String::new(),
            gametype: String::new(),
            num_players: 0,
            max_players: 0,
            password: false,
            version: String::new(),
            ping_ms: 0,
            players: Vec::new(),
            rules: IndexMap::new(),
            raw_name: String::new(),
            online: true,
            bots: 0,
            connect_port_deduced: false,
        }
    }

    pub fn humans(&self) -> i32 {
        (self.num_players.saturating_sub(self.bots)).max(0)
    }
}

// Como en Python: raw_name, online, bots y connect_port_deduced no cuentan.
impl PartialEq for ServerInfo {
    fn eq(&self, o: &Self) -> bool {
        self.address == o.address
            && self.connect_address == o.connect_address
            && self.name == o.name
            && self.map == o.map
            && self.gametype == o.gametype
            && self.num_players == o.num_players
            && self.max_players == o.max_players
            && self.password == o.password
            && self.version == o.version
            && self.ping_ms == o.ping_ms
            && self.players == o.players
            && self.rules == o.rules
    }
}
