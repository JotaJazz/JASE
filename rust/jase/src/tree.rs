//! Árbol de juegos (`game_tree.py`): cada juego con sus carpetas «Mods», «Mapas» y «Países»
//! de hojas marcables. El estado vive aquí y `rows()` da las filas visibles; la vista las
//! dibuja sin ítems propios (lo de `_render`/`_layout` se recalcula en cada `rows()`).

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::LazyLock;

use iced::widget::scrollable::Direction;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Space, container, mouse_area, row, scrollable, svg, text, tooltip};
use iced::{Center, Element, Fill};
use jase_core::browse::{BASE_MOD, Breakdown};
use jase_core::games::game;
use jase_core::geo::country_name;
use jase_core::i18n::tr;

use crate::app::Message;
use crate::flags::{FLAG_SIZE, flag};
use crate::icons::badge;
use crate::theme::{Palette, WidgetStyle, framed, hex, palette, style, widget_style};
use crate::widgets;

/// Sangría por nivel (la de `QTreeView`).
pub const INDENT: u32 = 20;
const KINDS: [Kind; 3] = [Kind::Mods, Kind::Maps, Kind::Countries];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Mods,
    Maps,
    Countries,
}

/// Nodo que se abre y se cierra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Node {
    Game,
    Folder(Kind),
}

/// Qué es cada fila: juego, carpeta, hoja marcable (con su valor) u «otros…».
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    Game,
    Folder(Kind),
    Leaf(Kind, String),
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub mods: HashSet<String>,
    pub maps: HashSet<String>,
    pub countries: HashSet<String>,
}

impl Marks {
    pub fn get(&self, kind: Kind) -> &HashSet<String> {
        match kind {
            Kind::Mods => &self.mods,
            Kind::Maps => &self.maps,
            Kind::Countries => &self.countries,
        }
    }

    fn get_mut(&mut self, kind: Kind) -> &mut HashSet<String> {
        match kind {
            Kind::Mods => &mut self.mods,
            Kind::Maps => &mut self.maps,
            Kind::Countries => &mut self.countries,
        }
    }

    pub fn is_empty(&self) -> bool {
        KINDS.iter().all(|&k| self.get(k).is_empty())
    }
}

fn list(b: &Breakdown, kind: Kind) -> &Vec<(String, usize)> {
    match kind {
        Kind::Mods => &b.mods,
        Kind::Maps => &b.maps,
        Kind::Countries => &b.countries,
    }
}

fn list_mut(b: &mut Breakdown, kind: Kind) -> &mut Vec<(String, usize)> {
    match kind {
        Kind::Mods => &mut b.mods,
        Kind::Maps => &mut b.maps,
        Kind::Countries => &mut b.countries,
    }
}

fn folder_title(kind: Kind) -> String {
    match kind {
        Kind::Mods => tr("Mods"),
        Kind::Maps => tr("Mapas"),
        Kind::Countries => tr("Países"),
    }
}

/// Fila visible del árbol, con lo que hace falta para dibujarla y sus ramas.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    pub depth: usize,
    pub item: Item,
    /// Juego dueño de la fila.
    pub game: &'static str,
    pub text: String,
    pub tooltip: String,
    /// `Some` solo en las hojas marcables.
    pub checked: Option<bool>,
    pub expandable: bool,
    pub expanded: bool,
    /// Por cada ancestro (juego, carpeta): ¿le sigue un hermano? (línea vertical en 98/XP).
    pub ancestors_have_next: Vec<bool>,
    pub has_next: bool,
    /// Primer juego: su línea empieza a media fila.
    pub first: bool,
    /// Código del país (hojas de «Países»).
    pub flag: Option<String>,
    /// El ítem actual (el que mueven las flechas).
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Space,
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// Clic o Enter en un juego.
    Activate(&'static str),
    /// Marca o desmarca una hoja.
    Toggle(&'static str, Kind, String),
    /// Abre o cierra un nodo (clic en la rama o en la carpeta).
    Expand(&'static str, Node),
    Nav(Nav),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    GameActivated(&'static str),
    MarksChanged(&'static str),
}

#[derive(Default)]
pub struct GameTree {
    ids: Vec<&'static str>,
    labels: HashMap<&'static str, (String, String)>,
    /// Desglose ya combinado con las marcas sin podar; sin entrada = sin carpetas.
    breakdown: HashMap<&'static str, Breakdown>,
    marks: HashMap<&'static str, Marks>,
    expanded: HashSet<(&'static str, Node)>,
    /// Juego actual (el de la tabla).
    current: Option<&'static str>,
    /// Ítem actual; si no está visible, las flechas parten del juego actual.
    cursor: Option<(&'static str, Item)>,
}

impl GameTree {
    pub fn game_ids(&self) -> Vec<&'static str> {
        self.ids.clone()
    }

    /// Reordena o filtra las filas; marcas, textos y expansión siguen por id.
    pub fn set_games(&mut self, ids: Vec<&'static str>) {
        self.ids = ids;
    }

    pub fn set_label(
        &mut self,
        id: &'static str,
        text: impl Into<String>,
        tooltip: impl Into<String>,
    ) {
        self.labels.insert(id, (text.into(), tooltip.into()));
    }

    /// `None` quita las carpetas pero nunca las marcas. Con `prune` se descartan las marcas
    /// de valores ausentes (devuelve si hubo alguna); sin él quedan como hojas con 0 (p. ej.
    /// mientras se refresca la lista).
    pub fn set_breakdown(&mut self, id: &'static str, b: Option<Breakdown>, prune: bool) -> bool {
        let Some(mut b) = b else {
            self.breakdown.remove(id);
            return false;
        };
        let marks = self.marks.entry(id).or_default();
        let mut dropped = false;
        for kind in KINDS {
            let shown = list_mut(&mut b, kind);
            let mut gone: Vec<String> = marks
                .get(kind)
                .iter()
                .filter(|v| !shown.iter().any(|(k, _)| k == *v))
                .cloned()
                .collect();
            gone.sort();
            if prune {
                dropped |= !gone.is_empty();
                for v in &gone {
                    marks.get_mut(kind).remove(v);
                }
            } else {
                shown.extend(gone.into_iter().map(|v| (v, 0)));
            }
        }
        self.breakdown.insert(id, b);
        dropped
    }

    /// Juego actual puesto desde la app: no se activa nada.
    pub fn set_current(&mut self, id: Option<&'static str>) {
        self.current = id;
        self.cursor = id.map(|g| (g, Item::Game));
    }

    pub fn current(&self) -> Option<&'static str> {
        self.current
    }

    pub fn marks(&self, id: &str) -> &Marks {
        static EMPTY: LazyLock<Marks> = LazyLock::new(Marks::default);
        self.marks.get(id).unwrap_or(&EMPTY)
    }

    pub fn clear_marks(&mut self, id: &'static str) {
        self.marks.remove(id);
    }

    pub fn set_expanded(&mut self, id: &'static str, node: Node, open: bool) {
        if open {
            self.expanded.insert((id, node));
        } else {
            self.expanded.remove(&(id, node));
        }
    }

    /// Carpetas del juego y sus hijos `(ítem, texto, marcada, bandera)`, como `_layout`.
    #[allow(clippy::type_complexity)]
    fn folders(&self, id: &str) -> Vec<(Kind, Vec<(Item, String, Option<bool>, Option<String>)>)> {
        let Some(b) = self.breakdown.get(id) else {
            return Vec::new();
        };
        let marks = self.marks(id);
        let mut out = Vec::new();
        for kind in KINDS {
            let values = list(b, kind);
            if kind != Kind::Maps && values.is_empty() {
                continue;
            }
            let mut leaves: Vec<_> = values
                .iter()
                .map(|(v, n)| {
                    let shown = match kind {
                        Kind::Countries => country_name(Some(v)),
                        Kind::Mods if v == BASE_MOD => tr("(base)"),
                        _ => v.clone(),
                    };
                    let flag = (kind == Kind::Countries).then(|| v.clone());
                    let item = Item::Leaf(kind, v.clone());
                    (
                        item,
                        format!("{shown} · {n}"),
                        Some(marks.get(kind).contains(v)),
                        flag,
                    )
                })
                .collect();
            if kind == Kind::Maps && b.other_maps > 0 {
                let text = format!("{} · {}", tr("otros…"), b.other_maps);
                leaves.push((Item::Other, text, None, None));
            }
            out.push((kind, leaves));
        }
        out
    }

    pub fn rows(&self) -> Vec<TreeRow> {
        let mut out = Vec::new();
        for (i, &id) in self.ids.iter().enumerate() {
            let game_next = i + 1 < self.ids.len();
            // Con desglose siempre hay carpetas («Mapas» va aunque esté vacía): las de un
            // juego cerrado no se arman.
            let expandable = self.breakdown.contains_key(id);
            let open = expandable && self.expanded.contains(&(id, Node::Game));
            let folders = if open { self.folders(id) } else { Vec::new() };
            let (text, tooltip) = self
                .labels
                .get(id)
                .cloned()
                .unwrap_or_else(|| (id.to_owned(), String::new()));
            let row = |depth, item: Item, text| TreeRow {
                selected: self.cursor.as_ref() == Some(&(id, item.clone())),
                depth,
                item,
                game: id,
                text,
                tooltip: String::new(),
                checked: None,
                expandable: false,
                expanded: false,
                ancestors_have_next: Vec::new(),
                has_next: false,
                first: false,
                flag: None,
            };
            out.push(TreeRow {
                tooltip,
                expandable,
                expanded: open,
                has_next: game_next,
                first: i == 0,
                ..row(0, Item::Game, text)
            });
            if !open {
                continue;
            }
            let n = folders.len();
            for (j, (kind, leaves)) in folders.into_iter().enumerate() {
                let folder_next = j + 1 < n;
                let f_open =
                    !leaves.is_empty() && self.expanded.contains(&(id, Node::Folder(kind)));
                out.push(TreeRow {
                    expandable: !leaves.is_empty(),
                    expanded: f_open,
                    ancestors_have_next: vec![game_next],
                    has_next: folder_next,
                    ..row(1, Item::Folder(kind), folder_title(kind))
                });
                if !f_open {
                    continue;
                }
                let m = leaves.len();
                for (k, (item, text, checked, flag)) in leaves.into_iter().enumerate() {
                    out.push(TreeRow {
                        checked,
                        ancestors_have_next: vec![game_next, folder_next],
                        has_next: k + 1 < m,
                        flag,
                        ..row(2, item, text)
                    });
                }
            }
        }
        out
    }

    fn toggle(&mut self, id: &'static str, kind: Kind, value: String) -> Option<Event> {
        let set = self.marks.entry(id).or_default().get_mut(kind);
        if !set.remove(&value) {
            set.insert(value.clone());
        }
        self.cursor = Some((id, Item::Leaf(kind, value)));
        // Una hoja de otro juego lo activa primero (con la marca ya puesta).
        if self.current != Some(id) {
            self.current = Some(id);
            return Some(Event::GameActivated(id));
        }
        Some(Event::MarksChanged(id))
    }

    /// Mueve el ítem actual; caer en un juego lo elige (como `currentItemChanged`).
    fn move_to(&mut self, r: &TreeRow) -> Option<Event> {
        self.cursor = Some((r.game, r.item.clone()));
        if r.item == Item::Game {
            self.current = Some(r.game);
            return Some(Event::GameActivated(r.game));
        }
        None
    }

    fn navigate(&mut self, nav: Nav) -> Option<Event> {
        let rows = self.rows();
        let pos = rows.iter().position(|r| r.selected).or_else(|| {
            let g = self.current?;
            rows.iter()
                .position(|r| r.game == g && r.item == Item::Game)
        });
        let Some(pos) = pos else {
            return match nav {
                Nav::Down => rows.first().and_then(|r| self.move_to(r)),
                _ => None,
            };
        };
        let r = &rows[pos];
        let node = match r.item {
            Item::Game => Some(Node::Game),
            Item::Folder(k) => Some(Node::Folder(k)),
            _ => None,
        };
        match nav {
            Nav::Up => pos.checked_sub(1).and_then(|p| self.move_to(&rows[p])),
            Nav::Down => rows.get(pos + 1).and_then(|r| self.move_to(r)),
            Nav::Left => match node {
                Some(n) if r.expanded => {
                    self.set_expanded(r.game, n, false);
                    None
                }
                // Al padre: la fila anterior más cercana con un nivel menos.
                _ => rows[..pos]
                    .iter()
                    .rposition(|p| p.depth + 1 == r.depth)
                    .and_then(|p| self.move_to(&rows[p])),
            },
            Nav::Right => match node {
                Some(n) if r.expandable && !r.expanded => {
                    self.set_expanded(r.game, n, true);
                    None
                }
                Some(_) if r.expanded => rows.get(pos + 1).and_then(|r| self.move_to(r)),
                _ => None,
            },
            Nav::Space => match &r.item {
                Item::Leaf(kind, v) => self.toggle(r.game, *kind, v.clone()),
                _ => None,
            },
        }
    }
}

pub fn update(t: &mut GameTree, msg: Msg) -> Option<Event> {
    match msg {
        Msg::Activate(id) => {
            t.current = Some(id);
            t.cursor = Some((id, Item::Game));
            Some(Event::GameActivated(id))
        }
        Msg::Toggle(id, kind, value) => t.toggle(id, kind, value),
        Msg::Expand(id, node) => {
            let open = !t.expanded.contains(&(id, node));
            t.set_expanded(id, node, open);
            t.cursor = Some((
                id,
                match node {
                    Node::Game => Item::Game,
                    Node::Folder(k) => Item::Folder(k),
                },
            ));
            None
        }
        Msg::Nav(nav) => t.navigate(nav),
    }
}

/// Alto de cada fila: el de los ítems de la lista en cada estilo (medido en 1.3.0).
pub fn row_height(s: WidgetStyle) -> u32 {
    match s {
        WidgetStyle::Flat => 26,
        WidgetStyle::Win98 | WidgetStyle::Luna | WidgetStyle::Vgui => 18,
        WidgetStyle::Terminal => 16,
    }
}

fn svg_open(w: u32, h: u32) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" shape-rendering=\"crispEdges\">"
    )
}

/// Puntos alternos de (x1, y1) a (x2, y2), alineados a la paridad x + y (`_dotted`).
fn dotted(out: &mut String, color: &str, (x1, y1): (u32, u32), (x2, y2): (u32, u32)) {
    for x in x1..=x2 {
        for y in y1..=y2 {
            if (x + y) % 2 == 0 {
                let _ = write!(
                    out,
                    "<rect x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\" fill=\"{color}\"/>"
                );
            }
        }
    }
}

/// Ramas de la fila, del ancho de su sangría (`drawBranches`): 98 y XP con líneas de puntos
/// `tree_line` y cuadro `tree_box` con − / +; el resto, solo la flecha `arrow` (▸ cerrado,
/// ▾ abierto) en los nodos con hijos. `None` = nada que dibujar.
pub fn branch_svg(r: &TreeRow, p: &Palette, s: WidgetStyle) -> Option<String> {
    let (w, h) = ((r.depth as u32 + 1) * INDENT, row_height(s));
    let (mid, half) = (h / 2, INDENT / 2);
    let mut out = svg_open(w, h);
    if !matches!(s, WidgetStyle::Win98 | WidgetStyle::Luna) {
        if !r.expandable {
            return None;
        }
        let (cx, cy) = (w - half, mid);
        let pts = if r.expanded {
            [(cx - 4, cy - 2), (cx + 4, cy - 2), (cx, cy + 3)]
        } else {
            [(cx - 2, cy - 4), (cx - 2, cy + 4), (cx + 3, cy)]
        };
        let [(ax, ay), (bx, by), (cx, cy)] = pts;
        let _ = write!(
            out,
            "<path d=\"M{ax} {ay}L{bx} {by}L{cx} {cy}Z\" fill=\"{}\" \
             shape-rendering=\"geometricPrecision\"/></svg>",
            hex(p.arrow)
        );
        return Some(out);
    }
    let dots = hex(p.tree_line);
    for (d, &next) in r.ancestors_have_next.iter().enumerate() {
        if next {
            let x = d as u32 * INDENT + half;
            dotted(&mut out, &dots, (x, 0), (x, h - 1));
        }
    }
    let x = r.depth as u32 * INDENT + half;
    let top = if r.first { mid } else { 0 };
    let bottom = if r.has_next { h - 1 } else { mid };
    dotted(&mut out, &dots, (x, top), (x, bottom));
    dotted(&mut out, &dots, (x, mid), (x + half, mid));
    if r.expandable {
        // Cuadro de 9×9: borde `tree_box` y degradado (en XP) de `surface` a `tree_box_bottom`.
        let (bx, by) = (x - 4, mid - 4);
        let _ = write!(
            out,
            "<defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
             <stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/>\
             </linearGradient></defs>\
             <rect x=\"{bx}\" y=\"{by}\" width=\"9\" height=\"9\" fill=\"{}\"/>\
             <rect x=\"{}\" y=\"{}\" width=\"7\" height=\"7\" fill=\"url(#g)\"/>\
             <rect x=\"{}\" y=\"{mid}\" width=\"5\" height=\"1\" fill=\"{}\"/>",
            hex(p.surface),
            hex(p.tree_box_bottom),
            hex(p.tree_box),
            bx + 1,
            by + 1,
            x - 2,
            hex(p.text),
        );
        if !r.expanded {
            let _ = write!(
                out,
                "<rect x=\"{x}\" y=\"{}\" width=\"1\" height=\"5\" fill=\"{}\"/>",
                mid - 2,
                hex(p.text)
            );
        }
    }
    out.push_str("</svg>");
    Some(out)
}

/// Relleno de cada ítem (`QTreeWidget#game_tree::item { padding }`).
fn item_padding(s: WidgetStyle) -> [u16; 2] {
    match s {
        WidgetStyle::Flat => [0, 8],
        WidgetStyle::Win98 | WidgetStyle::Luna | WidgetStyle::Terminal => [0, 3],
        WidgetStyle::Vgui => [0, 4],
    }
}

fn row_view<'a>(
    r: TreeRow,
    p: &'static Palette,
    s: WidgetStyle,
    theme_id: &'static str,
    focused: bool,
) -> Element<'a, Message> {
    let h = row_height(s) as f32;
    let label = |t: String| {
        text(t)
            .font(s.font())
            .size(s.text_size())
            .wrapping(Wrapping::None)
    };
    let indent = (r.depth as f32 + 1.0) * INDENT as f32;
    let branch: Element<'a, Message> = match branch_svg(&r, p, s) {
        Some(svg_text) => svg(svg::Handle::from_memory(svg_text.into_bytes()))
            .width(indent)
            .height(h)
            .into(),
        None => Space::new().width(indent).height(h).into(),
    };
    let node = match r.item {
        Item::Game => Some(Node::Game),
        Item::Folder(k) => Some(Node::Folder(k)),
        _ => None,
    };
    let branch = match node {
        Some(n) if r.expandable => mouse_area(branch)
            .on_press(Message::Tree(Msg::Expand(r.game, n)))
            .into(),
        _ => branch,
    };
    let (game_id, selected) = (r.game, r.selected);
    let content: Element<'a, Message> = match &r.item {
        Item::Game => {
            let icon = game(game_id).map_or_else(
                || Space::new().width(16).height(16).into(),
                |g| badge(g, p, 16.0),
            );
            row![icon, label(r.text.clone())]
                .spacing(4)
                .align_y(Center)
                .into()
        }
        Item::Leaf(kind, value) => {
            let flag_icon = r
                .flag
                .as_deref()
                .and_then(flag)
                .map(|h| svg(h).width(FLAG_SIZE.width).height(FLAG_SIZE.height));
            let text_row = row![]
                .push(flag_icon)
                .push(label(r.text.clone()))
                .spacing(4)
                .align_y(Center);
            let msg = Message::Tree(Msg::Toggle(game_id, *kind, value.clone()));
            widgets::check(
                r.checked == Some(true),
                Some(text_row.into()),
                Some(msg),
                p,
                s,
            )
        }
        Item::Folder(_) | Item::Other => label(r.text.clone()).into(),
    };
    let item = container(content)
        .width(Fill)
        .height(h)
        .center_y(h)
        .padding(item_padding(s))
        .clip(true)
        .style(move |_| style::list_item(p, s, selected, focused));
    let item: Element<'a, Message> = match style::list_item_edges(p, theme_id, selected) {
        Some(e) => framed(item, e),
        None => item.into(),
    };
    let item = match (&r.item, node) {
        (Item::Game, _) => mouse_area(item)
            .on_press(Message::Tree(Msg::Activate(game_id)))
            .into(),
        (_, Some(n)) => mouse_area(item)
            .on_press(Message::Tree(Msg::Expand(game_id, n)))
            .into(),
        _ => item,
    };
    let item = if r.tooltip.is_empty() {
        item
    } else {
        tooltip(
            item,
            container(text(r.tooltip).font(s.font()).size(s.text_size()))
                .padding(style::tooltip_padding(s))
                .style(move |_| style::tooltip(p, s)),
            tooltip::Position::Bottom,
        )
        .into()
    };
    row![branch, item].height(h).into()
}

/// Filas del árbol en un `scrollable`; el marco y el fondo los pone la barra lateral.
pub fn view<'a>(t: &'a GameTree, theme_id: &'static str, focused: bool) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let rows = Column::with_children(
        t.rows()
            .into_iter()
            .map(|r| row_view(r, p, s, theme_id, focused)),
    );
    // Como Qt, la barra ocupa su lugar cuando aparece.
    scrollable(rows)
        .direction(Direction::Vertical(style::scrollbar(s).spacing(0)))
        .style(move |_, status| style::scrollable(p, s, status))
        .height(Fill)
        .into()
}
