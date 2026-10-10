//! SyncTeX 正向（源码 → PDF）与反向（PDF → 源码）查询。

use std::path::{Path, PathBuf};

pub struct PdfLocation {
    pub page: u32,
    pub x: f32,
    pub y: f32,
}

pub fn forward(_synctex: &Path, _file: &Path, _line: u32) -> Option<PdfLocation> {
    None // TODO(M1)
}

pub fn inverse(_synctex: &Path, _loc: &PdfLocation) -> Option<(PathBuf, u32)> {
    None // TODO(M1)
}
