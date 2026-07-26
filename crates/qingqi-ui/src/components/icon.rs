//! Built-in, SVG-backed Lucide icons for GPUI.

use super::{Sizable, styled::Size};
use gpui::{
    App, Hsla, IntoElement, Pixels, RenderOnce, SharedString, Styled, Window,
    prelude::FluentBuilder, px, svg,
};

const DEFAULT_ICON_SIZE: Pixels = px(16.0);

#[derive(Clone, Debug, IntoElement)]
pub struct Icon {
    path: SharedString,
    size: Pixels,
    color: Option<Hsla>,
}

impl Icon {
    #[doc(hidden)]
    pub fn from_lucide_path(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            size: DEFAULT_ICON_SIZE,
            color: None,
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn text_color(self, color: impl Into<Hsla>) -> Self {
        self.color(color)
    }

    pub fn asset_path(&self) -> &str {
        self.path.as_ref()
    }
}

impl Sizable for Icon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = match size.into() {
            Size::Size(size) => size,
            Size::XSmall => px(12.0),
            Size::Small => px(14.0),
            Size::Medium => DEFAULT_ICON_SIZE,
            Size::Large => px(20.0),
        };
        self
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        svg()
            .path(self.path)
            .flex_none()
            .size(self.size)
            .when_some(self.color, |icon, color| icon.text_color(color))
    }
}

#[macro_export]
macro_rules! icon {
    ($name:ident) => {
        $crate::components::Icon::from_lucide_path($crate::__private::lucide_path!($name))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_use_lucide_namespace() {
        assert_eq!(crate::icon!(search).asset_path(), "lucide/search.svg");
        assert_eq!(
            crate::icon!(folder_open).asset_path(),
            "lucide/folder-open.svg"
        );
        assert_eq!(
            crate::icon!(triangle_alert).asset_path(),
            "lucide/triangle-alert.svg"
        );
        assert_eq!(crate::icon!(search).size, DEFAULT_ICON_SIZE);
        assert!(crate::icon!(search).color.is_none());

        let color = gpui::hsla(0.5, 0.8, 0.4, 1.0);
        let icon = crate::icon!(search).size(px(24.0)).color(color);
        assert_eq!(icon.size, px(24.0));
        assert_eq!(icon.color, Some(color));
    }

    #[test]
    fn every_generated_lucide_asset_is_loadable_and_tintable() {
        let paths = crate::assets::lucide_paths().collect::<Vec<_>>();
        assert!(paths.len() > 1_000, "expected the complete Lucide set");

        for path in paths {
            use gpui::AssetSource as _;

            let bytes = crate::assets::ProjectAssets
                .load(&path)
                .expect("ProjectAssets loads without error")
                .expect("generated Lucide path is embedded");
            let svg = std::str::from_utf8(&bytes).expect("Lucide SVG is UTF-8");
            assert!(
                svg.contains("viewBox=\"0 0 24 24\""),
                "invalid viewBox: {path}"
            );
            assert!(
                svg.contains("currentColor"),
                "icon cannot be tinted: {path}"
            );
        }
    }

    #[test]
    fn embedded_assets_match_the_committed_icon_manifest() {
        use gpui::AssetSource as _;
        use std::collections::BTreeSet;

        let embedded = crate::assets::lucide_paths()
            .map(|path| {
                path.strip_prefix("lucide/")
                    .expect("Lucide path uses namespace")
                    .strip_suffix(".svg")
                    .expect("Lucide asset is SVG")
                    .to_string()
            })
            .collect::<BTreeSet<_>>();
        let manifest = include_str!("../../assets/lucide-icons.txt")
            .lines()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(embedded, manifest);

        let listed = crate::assets::ProjectAssets
            .list("lucide")
            .expect("Lucide namespace can be listed");
        assert_eq!(listed.len(), embedded.len());
    }
}
