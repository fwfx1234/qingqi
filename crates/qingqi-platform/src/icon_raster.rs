//! SVG 栅格化（build.rs 与运行时共用，不依赖主 crate）。

use std::path::Path;

use resvg::usvg::{self, Transform};
use tiny_skia::Pixmap;

pub fn rasterize_square(svg_bytes: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_data(svg_bytes, &options).map_err(|error| error.to_string())?;
    let svg_size = tree.size();
    let max_dim = svg_size.width().max(svg_size.height());
    let scale = size as f32 / max_dim / 1.08;
    let offset_x = (size as f32 - svg_size.width() * scale) * 0.5;
    let offset_y = (size as f32 - svg_size.height() * scale) * 0.5;

    let mut pixmap =
        Pixmap::new(size, size).ok_or_else(|| String::from("failed to allocate pixmap"))?;
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    resvg::render(
        &tree,
        Transform::from_translate(offset_x, offset_y).pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    Ok(unpremultiply_rgba(pixmap.data()))
}

pub fn rasterize_svg_file(svg_path: &Path, size: u32, png_path: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(svg_path).map_err(|error| format!("read {}: {error}", svg_path.display()))?;
    let rgba = rasterize_square(&bytes, size)?;
    let image = image::RgbaImage::from_raw(size, size, rgba)
        .ok_or_else(|| String::from("invalid raster dimensions"))?;
    if let Some(parent) = png_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    image
        .save(png_path)
        .map_err(|error| format!("write {}: {error}", png_path.display()))
}

fn unpremultiply_rgba(premultiplied: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(premultiplied.len());
    for chunk in premultiplied.chunks_exact(4) {
        let [r, g, b, a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let alpha = a as f32 / 255.0;
        out.push((r as f32 / alpha).round().min(255.0) as u8);
        out.push((g as f32 / alpha).round().min(255.0) as u8);
        out.push((b as f32 / alpha).round().min(255.0) as u8);
        out.push(a);
    }
    out
}
