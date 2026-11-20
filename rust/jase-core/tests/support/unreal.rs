//! Paquete Unreal mínimo (versión 68) para tests: el `PackageBuilder` de `test_maps_unreal.py`.

pub const RF_HAS_STACK: u32 = 0x0200_0000;

/// FCompactIndex de Unreal: signo + 6 bits, luego grupos de 7 bits.
pub fn compact(value: i64) -> Vec<u8> {
    let mut v = value.unsigned_abs();
    let mut first = if value < 0 { 0x80 } else { 0 } | (v & 0x3F) as u8;
    v >>= 6;
    if v != 0 {
        first |= 0x40;
    }
    let mut out = vec![first];
    while v != 0 {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        out.push(b | if v != 0 { 0x80 } else { 0 });
    }
    out
}

struct Export {
    class: i64,
    name: i64,
    flags: u32,
    body: Vec<u8>,
}

#[derive(Default)]
pub struct PackageBuilder {
    names: Vec<String>,
    imports: Vec<[String; 3]>,
    exports: Vec<Export>,
}

impl PackageBuilder {
    pub fn new() -> Self {
        Self {
            names: vec!["None".into()],
            ..Self::default()
        }
    }

    pub fn name(&mut self, text: &str) -> i64 {
        if !self.names.iter().any(|n| n == text) {
            self.names.push(text.into());
        }
        self.names.iter().position(|n| n == text).unwrap() as i64
    }

    fn class_ref(&mut self, class_name: &str) -> i64 {
        for key in ["Core", "Class", class_name] {
            self.name(key);
        }
        let entry = ["Core".into(), "Class".into(), class_name.to_owned()];
        if !self.imports.contains(&entry) {
            self.imports.push(entry.clone());
        }
        -(self.imports.iter().position(|e| *e == entry).unwrap() as i64 + 1)
    }

    /// Agrega un export y devuelve su referencia (índice + 1).
    pub fn add_export(&mut self, class: &str, obj: &str, body: Vec<u8>, flags: u32) -> i64 {
        let class = self.class_ref(class);
        let name = self.name(obj);
        self.exports.push(Export {
            class,
            name,
            flags,
            body,
        });
        self.exports.len() as i64
    }

    pub fn prop(&mut self, name: &str, ptype: u8, payload: &[u8]) -> Vec<u8> {
        let code = match payload.len() {
            1 => Some(0),
            2 => Some(1),
            4 => Some(2),
            12 => Some(3),
            16 => Some(4),
            _ => None,
        };
        let mut out = compact(self.name(name));
        match code {
            Some(c) => out.push(ptype | (c << 4)),
            None => out.extend([ptype | (5 << 4), payload.len() as u8]),
        }
        out.extend(payload);
        out
    }

    pub fn none(&mut self) -> Vec<u8> {
        compact(self.name("None"))
    }

    fn index(&self, text: &str) -> i64 {
        self.names.iter().position(|n| n == text).unwrap() as i64
    }

    pub fn build(&self) -> Vec<u8> {
        let header_size = 64u32;
        let mut names = Vec::new();
        for n in &self.names {
            names.extend(compact(n.len() as i64 + 1));
            names.extend(n.bytes());
            names.extend([0, 0, 0, 0, 0]);
        }
        let mut imports = Vec::new();
        for [cp, cn, on] in &self.imports {
            imports.extend(compact(self.index(cp)));
            imports.extend(compact(self.index(cn)));
            imports.extend(0i32.to_le_bytes());
            imports.extend(compact(self.index(on)));
        }
        let name_off = header_size;
        let import_off = name_off + names.len() as u32;
        let bodies_off = import_off + imports.len() as u32;
        let mut bodies: Vec<u8> = Vec::new();
        let mut table = Vec::new();
        for e in &self.exports {
            let offset = bodies_off as i64 + bodies.len() as i64;
            bodies.extend(&e.body);
            table.extend(compact(e.class));
            table.extend(compact(0));
            table.extend(0i32.to_le_bytes());
            table.extend(compact(e.name));
            table.extend(e.flags.to_le_bytes());
            table.extend(compact(e.body.len() as i64));
            table.extend(compact(offset));
        }
        let export_off = bodies_off + bodies.len() as u32;
        let mut out = Vec::new();
        out.extend(0x9E2A_83C1u32.to_le_bytes());
        out.extend(68u16.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        for v in [
            0,
            self.names.len() as u32,
            name_off,
            self.exports.len() as u32,
            export_off,
            self.imports.len() as u32,
            import_off,
        ] {
            out.extend(v.to_le_bytes());
        }
        out.resize(header_size as usize, 0);
        out.extend(names);
        out.extend(imports);
        out.extend(bodies);
        out.extend(table);
        out
    }
}

pub fn palette_body(b: &mut PackageBuilder, colors: &[[u8; 3]]) -> Vec<u8> {
    let mut out = b.none();
    out.extend(compact(256));
    for i in 0..256 {
        let [r, g, bl] = colors.get(i).copied().unwrap_or([0, 0, 0]);
        out.extend([r, g, bl, 0]);
    }
    out
}

pub fn texture_body(
    b: &mut PackageBuilder,
    width: i32,
    height: i32,
    pixels: &[u8],
    palette_ref: Option<i64>,
    fmt: Option<u8>,
) -> Vec<u8> {
    let mut head = Vec::new();
    if let Some(p) = palette_ref {
        head.extend(b.prop("Palette", 5, &compact(p)));
    }
    if let Some(f) = fmt {
        head.extend(b.prop("Format", 1, &[f]));
    }
    head.extend(b.prop("USize", 2, &width.to_le_bytes()));
    head.extend(b.prop("VSize", 2, &height.to_le_bytes()));
    head.extend(b.none());
    head.extend(compact(1));
    let mut data = compact(pixels.len() as i64);
    data.extend(pixels);
    // skip offset: posición al final de los datos del mip (body_offset = 0)
    let skip = (head.len() + 4 + data.len()) as i32;
    head.extend(skip.to_le_bytes());
    head.extend(data);
    head.extend(width.to_le_bytes());
    head.extend(height.to_le_bytes());
    head.extend([1, 1]);
    head
}

pub const RED: [u8; 3] = [255, 0, 0];
pub const GREEN: [u8; 3] = [0, 255, 0];
pub const BLUE: [u8; 3] = [0, 0, 255];
pub const WHITE: [u8; 3] = [255, 255, 255];

/// Bytes de un mapa con la captura 2×2 (rojo, verde, azul, blanco) en P8.
pub fn make_map(with_levelinfo: bool, name: &str) -> Vec<u8> {
    let mut b = PackageBuilder::new();
    let body = palette_body(&mut b, &[RED, GREEN, BLUE, WHITE]);
    let pal = b.add_export("Palette", "Palette0", body, 0);
    let tex_name = if with_levelinfo { name } else { "Screenshot" };
    let body = texture_body(&mut b, 2, 2, &[0, 1, 2, 3], Some(pal), None);
    let tex = b.add_export("Texture", tex_name, body, 0);
    if with_levelinfo {
        // LevelInfo con stack (Node != 0 → offset extra) como en los mapas reales
        let mut body = compact(-1);
        body.extend(compact(-1));
        body.extend((-1i64).to_le_bytes());
        body.extend(0i32.to_le_bytes());
        body.extend(compact(-1));
        let mut title = compact(5);
        title.extend(b"Test\0");
        body.extend(b.prop("Title", 13, &title));
        body.extend(b.prop("Screenshot", 5, &compact(tex)));
        body.extend(b.none());
        b.add_export("LevelInfo", "LevelInfo0", body, RF_HAS_STACK);
    }
    b.build()
}
