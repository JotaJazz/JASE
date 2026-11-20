//! Imágenes de los mapas desde los archivos del juego. Traducción del paquete `maps/` de Python.
#[cfg(feature = "map-images")]
mod bcn;
pub mod cod;
#[cfg(feature = "map-images")]
pub mod iwi;
pub mod levelshots;
pub mod service;
pub mod unreal;
