//! SVG 栅格化（build.rs 与运行时共用，不依赖主 crate）。

use std::path::Path;

use resvg::usvg::{self, Transform};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap};

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

#[allow(dead_code)] // build.rs includes this module but only runtime code uses Dock rendering.
pub fn rasterize_dock_icon(
    svg_bytes: &[u8],
    size: u32,
    foreground: [u8; 3],
    background: [u8; 3],
) -> Result<Vec<u8>, String> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    options.style_sheet = Some(format!(
        "svg {{ color: #{:02x}{:02x}{:02x}; }}",
        foreground[0], foreground[1], foreground[2]
    ));
    let tree = usvg::Tree::from_data(svg_bytes, &options).map_err(|error| error.to_string())?;

    let mut pixmap = Pixmap::new(size, size)
        .ok_or_else(|| String::from("failed to allocate Dock icon pixmap"))?;
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    let inset = size as f32 * 0.075;
    let radius = size as f32 * 0.2;
    let mut paint = Paint::default();
    paint.set_color_rgba8(background[0], background[1], background[2], 255);
    let background_path = rounded_rect_path(
        inset,
        inset,
        size as f32 - inset,
        size as f32 - inset,
        radius,
    )?;
    pixmap.fill_path(
        &background_path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let svg_size = tree.size();
    let max_dim = svg_size.width().max(svg_size.height());
    let glyph_size = size as f32 * 0.5;
    let scale = glyph_size / max_dim;
    let offset_x = (size as f32 - svg_size.width() * scale) * 0.5;
    let offset_y = (size as f32 - svg_size.height() * scale) * 0.5;
    resvg::render(
        &tree,
        Transform::from_translate(offset_x, offset_y).pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let rgba = unpremultiply_rgba(pixmap.data());
    let image = image::RgbaImage::from_raw(size, size, rgba)
        .ok_or_else(|| String::from("invalid Dock icon dimensions"))?;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| format!("encode Dock icon PNG: {error}"))?;
    Ok(png.into_inner())
}

#[allow(dead_code)]
fn rounded_rect_path(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
) -> Result<tiny_skia::Path, String> {
    let radius = radius.min((right - left) * 0.5).min((bottom - top) * 0.5);
    let control = radius * 0.552_284_8;
    let mut path = PathBuilder::new();
    path.move_to(left + radius, top);
    path.line_to(right - radius, top);
    path.cubic_to(
        right - radius + control,
        top,
        right,
        top + radius - control,
        right,
        top + radius,
    );
    path.line_to(right, bottom - radius);
    path.cubic_to(
        right,
        bottom - radius + control,
        right - radius + control,
        bottom,
        right - radius,
        bottom,
    );
    path.line_to(left + radius, bottom);
    path.cubic_to(
        left + radius - control,
        bottom,
        left,
        bottom - radius + control,
        left,
        bottom - radius,
    );
    path.line_to(left, top + radius);
    path.cubic_to(
        left,
        top + radius - control,
        left + radius - control,
        top,
        left + radius,
        top,
    );
    path.close();
    path.finish()
        .ok_or_else(|| String::from("failed to build Dock icon background"))
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

#[cfg(test)]
mod tests {
    use super::rasterize_dock_icon;

    #[test]
    fn dock_icon_is_tinted_png_with_transparent_corners() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><circle cx="12" cy="12" r="7"/></svg>"#;
        let png = rasterize_dock_icon(svg, 512, [220, 38, 38], [220, 252, 231])
            .expect("Dock icon should render");
        let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .expect("Dock icon should be a PNG")
            .into_rgba8();

        assert_eq!(image.dimensions(), (512, 512));
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert!(image.pixels().any(|pixel| {
            let [r, g, b, a] = pixel.0;
            a > 0 && r > 180 && g < 100 && b < 100
        }));
    }
}
