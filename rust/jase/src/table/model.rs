//! Modelo de la tabla de servidores (`server_model.py`): textos de celda y orden.

use std::cmp::Ordering;
use std::collections::HashMap;

use jase_core::i18n::tr;
use jase_core::models::{ServerAddress, ServerInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    Lock,
    Country,
    Name,
    Ping,
    Players,
    Map,
    Mode,
    Version,
}

impl Column {
    pub const ALL: [Column; 8] = [
        Column::Lock,
        Column::Country,
        Column::Name,
        Column::Ping,
        Column::Players,
        Column::Map,
        Column::Mode,
        Column::Version,
    ];

    /// Cabecera traducida; se resuelve al pedirla (el idioma se fija después).
    pub fn title(self) -> String {
        match self {
            Column::Lock => String::new(),
            Column::Country => tr("País"),
            Column::Name => tr("Servidor"),
            Column::Ping => tr("Ping"),
            Column::Players => tr("Jugadores"),
            Column::Map => tr("Mapa"),
            Column::Mode => tr("Modo"),
            Column::Version => tr("Versión"),
        }
    }

    /// Ancho inicial en px (`COLUMN_WIDTHS` de `main_window.py`); "Servidor" ocupa el resto.
    pub fn initial_width(self) -> Option<f32> {
        match self {
            Column::Lock => Some(28.0),
            Column::Country => Some(48.0),
            Column::Name => None,
            Column::Ping => Some(70.0),
            Column::Players => Some(90.0),
            Column::Map => Some(170.0),
            Column::Mode => Some(150.0),
            Column::Version => Some(70.0),
        }
    }
}

/// Valor de orden del ping de una fila offline: queda al final en orden ascendente.
pub const OFFLINE_PING_SORT: i64 = (1 << 31) - 1;

/// «8/16», o «2+6/16» con bots (2 personas y 6 bots).
pub fn players_text(info: &ServerInfo) -> String {
    if info.bots != 0 {
        format!("{}+{}/{}", info.humans(), info.bots, info.max_players)
    } else {
        format!("{}/{}", info.num_players, info.max_players)
    }
}

/// Fracción ocupada en [0, 1]; 0 si el máximo es desconocido (<= 0).
pub fn occupancy_ratio(num: i32, max: i32) -> f32 {
    if max <= 0 {
        return 0.0;
    }
    (num as f32 / max as f32).clamp(0.0, 1.0)
}

pub fn cell_text(info: &ServerInfo, col: Column, version_label: &dyn Fn(&str) -> String) -> String {
    match col {
        Column::Lock if info.password => "🔒".to_owned(),
        Column::Lock | Column::Country => String::new(), // País: solo bandera
        Column::Name if info.online => info.name.clone(),
        Column::Name => info.name.clone() + &tr(" — sin respuesta"),
        Column::Ping if info.online => info.ping_ms.to_string(),
        Column::Players if info.online => players_text(info),
        Column::Ping | Column::Players => "—".to_owned(),
        Column::Map => info.map.clone(),
        Column::Mode => info.gametype.clone(),
        Column::Version => version_label(&info.version),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SortKey {
    Int(i64),
    Text(String),
}

pub fn sort_key(info: &ServerInfo, col: Column, version_label: &dyn Fn(&str) -> String) -> SortKey {
    match col {
        Column::Ping if info.online => SortKey::Int(info.ping_ms.into()),
        Column::Ping => SortKey::Int(OFFLINE_PING_SORT),
        // Personas primero; a igualdad, cuenta el total.
        Column::Players => {
            SortKey::Int(i64::from(info.humans()) * 1024 + i64::from(info.num_players.min(1023)))
        }
        _ => SortKey::Text(cell_text(info, col, version_label).to_lowercase()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub ascending: bool,
}

/// Filas en orden de llegada y su orden de vista, mantenido al llegar cada fila.
pub struct TableModel {
    rows: Vec<ServerInfo>,
    index: HashMap<ServerAddress, usize>,
    /// Clave de cada fila para la columna de `sort`.
    keys: Vec<SortKey>,
    order: Vec<usize>,
    sort: Sort,
    version_label: Box<dyn Fn(&str) -> String>,
}

impl Default for TableModel {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            index: HashMap::new(),
            keys: Vec::new(),
            order: Vec::new(),
            // Como `sortByColumn(COL_PING, Ascending)` de la ventana Qt.
            sort: Sort {
                column: Column::Ping,
                ascending: true,
            },
            version_label: Box::new(str::to_owned),
        }
    }
}

impl TableModel {
    /// Clave total `(SortKey, llegada)`: a igual clave, la fila que llegó antes va primero.
    fn cmp_rows(&self, a: usize, b: usize) -> Ordering {
        let o = self.keys[a].cmp(&self.keys[b]);
        let o = if self.sort.ascending { o } else { o.reverse() };
        o.then(a.cmp(&b))
    }

    fn key(&self, info: &ServerInfo) -> SortKey {
        sort_key(info, self.sort.column, &*self.version_label)
    }

    fn place(&mut self, row: usize) {
        let at = self
            .order
            .partition_point(|&j| self.cmp_rows(j, row) == Ordering::Less);
        self.order.insert(at, row);
    }

    /// Agrega la fila o reemplaza la de la misma dirección; devuelve su fila.
    pub fn add_or_update(&mut self, info: ServerInfo) -> usize {
        let key = self.key(&info);
        let row = match self.index.get(&info.address) {
            Some(&row) => {
                self.order.retain(|&j| j != row);
                self.rows[row] = info;
                self.keys[row] = key;
                row
            }
            None => {
                let row = self.rows.len();
                self.index.insert(info.address.clone(), row);
                self.rows.push(info);
                self.keys.push(key);
                row
            }
        };
        self.place(row);
        row
    }

    pub fn remove(&mut self, address: &ServerAddress) {
        let Some(row) = self.index.remove(address) else {
            return;
        };
        self.rows.remove(row);
        self.keys.remove(row);
        // Las filas siguientes bajan una posición; el orden relativo no cambia.
        self.order.retain(|&j| j != row);
        for j in &mut self.order {
            if *j > row {
                *j -= 1;
            }
        }
        for j in self.index.values_mut() {
            if *j > row {
                *j -= 1;
            }
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
        self.index.clear();
        self.keys.clear();
        self.order.clear();
    }

    pub fn servers(&self) -> &[ServerInfo] {
        &self.rows
    }

    pub fn row_of(&self, address: &ServerAddress) -> Option<usize> {
        self.index.get(address).copied()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    /// Cambia el orden y reordena todo.
    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.resort();
    }

    /// Etiqueta de versión del juego actual (`GameDef::version_label`); reordena todo.
    pub fn set_version_label(&mut self, f: impl Fn(&str) -> String + 'static) {
        self.version_label = Box::new(f);
        self.resort();
    }

    pub fn version_label(&self, version: &str) -> String {
        (self.version_label)(version)
    }

    fn resort(&mut self) {
        self.keys = self.rows.iter().map(|info| self.key(info)).collect();
        let mut order: Vec<usize> = (0..self.rows.len()).collect();
        order.sort_by(|&a, &b| self.cmp_rows(a, b));
        self.order = order;
    }

    /// Filas de `servers()` en el orden de la vista.
    pub fn order(&self) -> &[usize] {
        &self.order
    }
}
