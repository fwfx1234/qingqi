//! 从 `assets/app-icon.svg` 生成 cargo-bundle 所需的 PNG 尺寸。

use std::path::PathBuf;

#[path = "../qingqi-platform/src/icon_raster.rs"]
mod icon_raster;

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let svg = manifest_dir.join("assets/app-icon.svg");
    println!("cargo:rerun-if-changed={}", svg.display());
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir
            .join("../qingqi-platform/src/icon_raster.rs")
            .display()
    );

    for size in [16_u32, 32, 64, 128, 256, 512] {
        let png = manifest_dir.join(format!("assets/app_icon_{size}.png"));
        if let Err(error) = icon_raster::rasterize_svg_file(&svg, size, &png) {
            panic!("app icon {size}px: {error}");
        }
    }
}
