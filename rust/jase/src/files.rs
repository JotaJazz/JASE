//! Selectores de archivo de *Exportar/Importar favoritos* (`QFileDialog` de
//! `window_favorites.py`): bloquean hasta que el usuario elige, así que solo corren dentro de
//! `app::blocking`; los tests ponen un falso.

use std::path::PathBuf;
use std::sync::Arc;

use jase_core::favorites_io::ImportError;
use serde_json::Value;

/// `None` = cancelado (o sin selector en el sistema: sin portal ni `zenity`).
pub trait FileDialogs: Send + Sync {
    fn save_json(&self, title: &str, filter: &str, file_name: &str) -> Option<PathBuf>;
    fn open_json(&self, title: &str, filter: &str) -> Option<PathBuf>;
}

pub type FilesFn = Arc<dyn FileDialogs>;

/// Los de `rfd`, sin ventana padre (`set_parent` pide el `HasWindowHandle` en este hilo): el
/// selector no es modal.
pub struct RfdDialogs;

impl FileDialogs for RfdDialogs {
    fn save_json(&self, title: &str, filter: &str, file_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(file_name)
            .add_filter(filter, &["json"])
            .save_file()
    }

    fn open_json(&self, title: &str, filter: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title(title)
            .add_filter(filter, &["json"])
            .pick_file()
    }
}

/// Sin selector (smoke y capturas): siempre cancelado.
pub struct NoFiles;

impl FileDialogs for NoFiles {
    fn save_json(&self, _: &str, _: &str, _: &str) -> Option<PathBuf> {
        None
    }

    fn open_json(&self, _: &str, _: &str) -> Option<PathBuf> {
        None
    }
}

/// Por qué no se importó (`ImportError` no es `Clone`: el de E/S va como texto).
#[derive(Debug, Clone, PartialEq)]
pub enum ImportFailure {
    Read(String),
    TooLarge,
    Invalid,
}

impl From<ImportError> for ImportFailure {
    fn from(e: ImportError) -> Self {
        match e {
            ImportError::TooLarge => ImportFailure::TooLarge,
            ImportError::Invalid => ImportFailure::Invalid,
            ImportError::Io(e) => ImportFailure::Read(e.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FilesMsg {
    /// La ruta elegida para exportar (`None` = cancelado).
    ExportTo(Option<PathBuf>),
    Exported {
        path: PathBuf,
        result: Result<(), String>,
    },
    /// El archivo elegido para importar (`None` = cancelado).
    ImportFrom(Option<PathBuf>),
    /// El JSON leído en un hilo.
    Imported {
        result: Result<Value, ImportFailure>,
    },
}
