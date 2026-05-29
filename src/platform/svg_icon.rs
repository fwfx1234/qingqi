//! 将 SVG 栅格化为 RGBA，供系统托盘等场景使用。

#[path = "../../icon_raster.rs"]
mod icon_raster;

pub use icon_raster::rasterize_square;

/// 从项目 assets 相对路径加载并栅格化 SVG。
pub fn rasterize_asset(relative: &str, size: u32) -> Result<Vec<u8>, String> {
    let path = crate::app::assets::resolve(relative);
    let bytes =
        std::fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    rasterize_square(&bytes, size)
}
