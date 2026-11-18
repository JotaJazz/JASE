//! Modelo de la tabla de servidores (`server_model.py`): textos de celda, país, filtro y orden.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use super::ServerFilter;

use jase_core::geo::country_name;
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

    /// Tooltip del encabezado (`headerData(ToolTipRole)`): la atribución de la base de países.
    pub fn header_tooltip(self) -> Option<String> {
        (self == Column::Country)
            .then(|| tr("País por IP: IP to Country Lite by DB-IP (db-ip.com), CC BY 4.0"))
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

/// `country`: código del servidor (minúsculas); País ordena por su nombre en el idioma actual.
pub fn sort_key(
    info: &ServerInfo,
    col: Column,
    version_label: &dyn Fn(&str) -> String,
    country: Option<&str>,
) -> SortKey {
    match col {
        Column::Country => SortKey::Text(
            country.map_or_else(String::new, |c| country_name(Some(c)).to_lowercase()),
        ),
        Column::Ping if info.online => SortKey::Int(info.ping_ms.into()),
        Column::Ping => SortKey::Int(OFFLINE_PING_SORT),
        // Personas primero; a igualdad, cuenta el total.
        Column::Players => {
            SortKey::Int(i64::from(info.humans()) * 1024 + i64::from(info.num_players.min(1023)))
        }
        _ => SortKey::Text(cell_text(info, col, version_label).to_lowercase()),
    }
}

/// IP → código de país (`set_country_lookup` de Python).
pub type CountryLookup = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub ascending: bool,
}

/// Filas en orden de llegada y su orden de vista (solo las que pasan el filtro), mantenido al
/// llegar cada fila.
pub struct TableModel {
    rows: Vec<ServerInfo>,
    index: HashMap<ServerAddress, usize>,
    /// País de cada fila, buscado una sola vez al llegar (o al cambiar la búsqueda).
    countries: Vec<Option<String>>,
    /// Clave de cada fila para la columna de `sort`.
    keys: Vec<SortKey>,
    order: Vec<usize>,
    sort: Sort,
    filter: ServerFilter,
    version_label: Box<dyn Fn(&str) -> String>,
    country_lookup: Option<CountryLookup>,
    /// Notas de favoritos del juego actual (el tooltip de *Servidor*).
    notes: IndexMap<ServerAddress, String>,
}

impl Default for TableModel {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            index: HashMap::new(),
            countries: Vec::new(),
            keys: Vec::new(),
            order: Vec::new(),
            // Como `sortByColumn(COL_PING, Ascending)` de la ventana Qt.
            sort: Sort {
                column: Column::Ping,
                ascending: true,
            },
            filter: ServerFilter::default(),
            version_label: Box::new(str::to_owned),
            country_lookup: None,
            notes: IndexMap::new(),
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

    fn key(&self, info: &ServerInfo, country: Option<&str>) -> SortKey {
        sort_key(info, self.sort.column, &*self.version_label, country)
    }

    /// Busca el país de un servidor que quizá no está en la tabla (el desglose de la lista
    /// guardada de otro juego); para las filas, `country_code`.
    pub fn lookup(&self, info: &ServerInfo) -> Option<String> {
        let code = (self.country_lookup.as_ref()?)(&info.address.host)?;
        (!code.is_empty()).then(|| code.to_lowercase())
    }

    fn accepted(&self, row: usize) -> bool {
        self.filter
            .accepts(&self.rows[row], self.countries[row].as_deref())
    }

    fn place(&mut self, row: usize) {
        if !self.accepted(row) {
            return;
        }
        let at = self
            .order
            .partition_point(|&j| self.cmp_rows(j, row) == Ordering::Less);
        self.order.insert(at, row);
    }

    /// Agrega la fila o reemplaza la de la misma dirección; devuelve su fila.
    pub fn add_or_update(&mut self, info: ServerInfo) -> usize {
        let country = self.lookup(&info);
        let key = self.key(&info, country.as_deref());
        let row = match self.index.get(&info.address) {
            Some(&row) => {
                self.order.retain(|&j| j != row);
                self.rows[row] = info;
                self.countries[row] = country;
                self.keys[row] = key;
                row
            }
            None => {
                let row = self.rows.len();
                self.index.insert(info.address.clone(), row);
                self.rows.push(info);
                self.countries.push(country);
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
        self.countries.remove(row);
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
        self.countries.clear();
        self.keys.clear();
        self.order.clear();
    }

    pub fn servers(&self) -> &[ServerInfo] {
        &self.rows
    }

    pub fn row_of(&self, address: &ServerAddress) -> Option<usize> {
        self.index.get(address).copied()
    }

    /// La fila de `address` existe y pasa el filtro.
    pub fn is_shown(&self, address: &ServerAddress) -> bool {
        self.row_of(address).is_some_and(|row| self.accepted(row))
    }

    /// Todas las filas, también las que oculta el filtro (`order().len()` cuenta las visibles).
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

    /// Notas de favoritos (se muestran en el tooltip de la columna Servidor).
    pub fn set_notes(&mut self, notes: IndexMap<ServerAddress, String>) {
        self.notes = notes;
    }

    pub fn note(&self, address: &ServerAddress) -> Option<&str> {
        self.notes.get(address).map(String::as_str)
    }

    /// Búsqueda IP → país (la base se carga fuera del hilo de la UI); `None` la quita.
    /// Vuelve a buscar el país de cada fila y reordena y refiltra todo.
    pub fn set_country_lookup(&mut self, lookup: Option<CountryLookup>) {
        self.country_lookup = lookup;
        self.countries = self.rows.iter().map(|info| self.lookup(info)).collect();
        self.resort();
    }

    /// Código ISO (minúsculas) guardado para la fila de `info`, o `None`.
    pub fn country_code(&self, info: &ServerInfo) -> Option<&str> {
        let row = self.row_of(&info.address)?;
        self.countries[row].as_deref()
    }

    pub fn filter(&self) -> &ServerFilter {
        &self.filter
    }

    /// Cambia el filtro y rehace `order` con las claves ya calculadas. La app pasa siempre por
    /// `table::set_filter`, que además suelta la selección que deja de verse.
    pub fn set_filter(&mut self, filter: ServerFilter) {
        self.filter = filter;
        self.refilter();
    }

    fn resort(&mut self) {
        self.keys = (0..self.rows.len())
            .map(|i| self.key(&self.rows[i], self.countries[i].as_deref()))
            .collect();
        self.refilter();
    }

    fn refilter(&mut self) {
        let mut order: Vec<usize> = (0..self.rows.len())
            .filter(|&row| self.accepted(row))
            .collect();
        order.sort_by(|&a, &b| self.cmp_rows(a, b));
        self.order = order;
    }

    /// Filas de `servers()` que pasan el filtro, en el orden de la vista.
    pub fn order(&self) -> &[usize] {
        &self.order
    }
}
