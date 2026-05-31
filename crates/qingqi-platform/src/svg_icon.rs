//! 将 SVG 栅格化为 RGBA，供系统托盘等场景使用。

pub use crate::icon_raster::rasterize_square;

/// 从已解析的绝对路径加载并栅格化 SVG。
pub fn rasterize_path(path: &std::path::Path, size: u32) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    rasterize_square(&bytes, size)
}
