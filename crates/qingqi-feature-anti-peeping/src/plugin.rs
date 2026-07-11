use std::path::Path;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use anyhow::Context as AnyhowContext;
use gpui::{
    AnyWindowHandle, App, AppContext, Context, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions, div, img, prelude::FluentBuilder, px,
};
use qingqi_plugin::{
    command::Command,
    plugin::{Manifest, Plugin, PluginCx, PluginId, PluginView, WindowView},
    storage::AppPaths,
};
use qingqi_ui::components::Button;
use qingqi_ui::icon;
use qingqi_ui::ui;

use super::manifest;

pub struct AntiPeepingPlugin {
    paths: AppPaths,
    image_path: Rc<RefCell<Option<String>>>,
    active: Rc<RefCell<bool>>,
    overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
    picker: Rc<dyn Fn() -> Option<PathBuf>>,
}

impl AntiPeepingPlugin {
    pub fn new(paths: AppPaths) -> Self {
        Self::with_picker(paths, Rc::new(default_image_picker))
    }

    pub fn with_picker(paths: AppPaths, picker: Rc<dyn Fn() -> Option<PathBuf>>) -> Self {
        let image_path = Rc::new(RefCell::new(Self::load_custom_image(&paths)));
        Self {
            paths,
            image_path,
            active: Rc::new(RefCell::new(false)),
            overlay_windows: Rc::new(RefCell::new(Vec::new())),
            picker,
        }
    }

    fn load_custom_image(paths: &AppPaths) -> Option<String> {
        let config_path = paths.config("anti-peeping.json");
        let content = std::fs::read_to_string(&config_path).ok()?;
        if content.trim().is_empty() {
            return None;
        }
        let value: serde_json::Value = serde_json::from_str(&content).ok()?;
        let raw = value.get("image_path")?;
        if raw.is_null() {
            return None;
        }
        let s = raw.as_str()?;
        if s.trim().is_empty() {
            return None;
        }
        Some(s.to_string())
    }

    fn save_custom_image(paths: &AppPaths, value: Option<&str>) -> anyhow::Result<()> {
        if let Some(p) = value {
            let path = Path::new(p);
            if !path.is_file() {
                return Err(anyhow::anyhow!("路径不是文件: {}", p));
            }
            validate_image_content(path).map_err(|err| anyhow::anyhow!("图片内容无效: {err}"))?;
        }

        let config_path = paths.config("anti-peeping.json");
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| std::format!("创建防窥配置目录失败: {}", parent.display()))?;
        }
        let value = match value {
            Some(v) if !v.trim().is_empty() => serde_json::Value::String(v.to_string()),
            _ => serde_json::Value::Null,
        };
        let document = serde_json::json!({"image_path": value});
        let serialized = serde_json::to_string_pretty(&document).context("序列化防窥配置失败")?;
        std::fs::write(&config_path, serialized)
            .with_context(|| std::format!("写入防窥配置失败: {}", config_path.display()))?;
        Ok(())
    }

    fn open_overlays(
        cx: &mut App,
        image_path: Rc<RefCell<Option<String>>>,
        overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
        active: Rc<RefCell<bool>>,
    ) {
        Self::close_overlays(cx, &overlay_windows);

        let displays: Vec<_> = cx.displays();
        for display in &displays {
            let bounds = display.bounds();
            let ip = Rc::clone(&image_path);
            let active = Rc::clone(&active);
            let handles = Rc::clone(&overlay_windows);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Fullscreen(bounds)),
                display_id: Some(display.id()),
                kind: WindowKind::PopUp,
                focus: true,
                show: true,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                titlebar: None,
                window_background: WindowBackgroundAppearance::Opaque,
                ..Default::default()
            };
            match cx.open_window(options, move |window, cx| {
                cx.new(|cx| {
                    let overlay = AntiPeepingOverlay::new(ip, active, handles, cx);
                    window.focus(&overlay.focus_handle);
                    overlay
                })
            }) {
                Ok(handle) => overlay_windows.borrow_mut().push(handle.into()),
                Err(error) => tracing::warn!(error = %error, "open anti-peeping overlay failed"),
            }
        }
    }

    fn close_overlays(cx: &mut App, overlay_windows: &Rc<RefCell<Vec<AnyWindowHandle>>>) {
        for handle in overlay_windows.borrow_mut().drain(..) {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
}

/// 默认图片选择器。
fn default_image_picker() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("选择防窥自定义图片")
        .add_filter("图片", &["png", "jpg", "jpeg", "bmp", "webp"])
        .set_file_name("anti-peeping.png")
        .pick_file()
}

/// 校验路径可被无损存入 JSON 配置：
/// 1. 必须是普通文件；
/// 2. 必须是 UTF-8（否则 serde_json 无法无损表示）；
/// 3. 按 image 解码校验内容，而非只信扩展名。
fn validate_image_path(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Err("选择的路径不是文件".to_string());
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| "不支持该路径: 包含非 UTF-8 字符".to_string())?;
    validate_image_content(path)?;
    Ok(path_str.to_string())
}

fn validate_image_content(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|err| std::format!("无法读取图片: {err}"))?;
    image::load_from_memory(&bytes).map_err(|err| std::format!("图片内容无法解码: {err}"))?;
    Ok(())
}

/// 模拟「选择图片」按钮的点击回调：校验 picker 结果，写入草稿或错误。
/// cancel（None）不改变草稿。
fn apply_picked_image(
    draft: &RefCell<String>,
    error: &RefCell<Option<String>>,
    picked: Option<PathBuf>,
) {
    let Some(path) = picked else {
        return;
    };
    match validate_image_path(&path) {
        Ok(validated) => {
            *draft.borrow_mut() = validated;
            *error.borrow_mut() = None;
        }
        Err(err) => {
            *error.borrow_mut() = Some(err);
        }
    }
}

/// 模拟「清除图片」按钮的点击回调：只重置草稿与错误，不改变已保存的配置。
fn apply_clear(draft: &RefCell<String>, error: &RefCell<Option<String>>) {
    *draft.borrow_mut() = String::new();
    *error.borrow_mut() = None;
}

/// 模拟「保存设置」按钮的点击回调：把草稿持久化并更新运行时 image_path。
fn apply_save(
    runtime_ip: &RefCell<Option<String>>,
    error: &RefCell<Option<String>>,
    paths: &AppPaths,
    draft: &RefCell<String>,
) -> Result<(), String> {
    let new_value = draft.borrow().clone();
    let new_value = if new_value.is_empty() {
        None
    } else {
        Some(new_value.as_str())
    };
    AntiPeepingPlugin::save_custom_image(paths, new_value)
        .map_err(|err| std::format!("保存失败: {err}"))?;
    *runtime_ip.borrow_mut() = new_value.map(String::from);
    *error.borrow_mut() = None;
    Ok(())
}

impl Plugin for AntiPeepingPlugin {
    fn manifest(&self) -> Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        let m = self.manifest();
        vec![Command::plugin_open(
            m.id.as_ref(),
            "打开防窥屏",
            "全屏遮盖所有屏幕内容",
            m.keywords.iter().map(|s| s.as_ref()),
            m.command_prefixes.iter().map(|s| s.as_ref()),
            m.icon.as_str(),
        )]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> anyhow::Result<PluginView> {
        if !*self.active.borrow() {
            *self.active.borrow_mut() = true;
            let image_path = Rc::clone(&self.image_path);
            let overlay_windows = Rc::clone(&self.overlay_windows);
            let active = Rc::clone(&self.active);
            Self::open_overlays(cx.app, image_path, overlay_windows, active);
        }
        let draft_path = self.image_path.borrow().clone().unwrap_or_default();
        Ok(PluginView::Window(Box::new(AntiPeepingView {
            active: Rc::clone(&self.active),
            image_path: Rc::clone(&self.image_path),
            overlay_windows: Rc::clone(&self.overlay_windows),
            paths: self.paths.clone(),
            draft_path: Rc::new(RefCell::new(draft_path)),
            draft_error: Rc::new(RefCell::new(None)),
            picker: Rc::clone(&self.picker),
        })))
    }

    fn close_idle(&mut self) {
        *self.active.borrow_mut() = false;
    }
}

struct AntiPeepingView {
    active: Rc<RefCell<bool>>,
    image_path: Rc<RefCell<Option<String>>>,
    overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
    paths: AppPaths,
    draft_path: Rc<RefCell<String>>,
    draft_error: Rc<RefCell<Option<String>>>,
    picker: Rc<dyn Fn() -> Option<PathBuf>>,
}

#[cfg(test)]
impl AntiPeepingView {
    // 测试辅助：模拟「选择图片」点击（用注入的 picker）。
    fn apply_pick_from_injected_input(&self) {
        apply_picked_image(&self.draft_path, &self.draft_error, (self.picker)());
    }

    // 测试辅助：模拟「清除图片」点击。
    fn apply_clear_click(&self) {
        apply_clear(&self.draft_path, &self.draft_error);
    }
}

impl WindowView for AntiPeepingView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "防窥屏".into()
    }

    fn render(&mut self, _window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let active = *self.active.borrow();
        let ip = Rc::clone(&self.image_path);
        let draft = Rc::clone(&self.draft_path);
        let error = Rc::clone(&self.draft_error);

        let draft_label = if draft.borrow().is_empty() {
            "（使用纯黑色）".to_string()
        } else {
            draft.borrow().clone()
        };
        let error_label = error.borrow().clone();

        div()
            .flex()
            .flex_col()
            .p(px(20.0))
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("防窥屏"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(ui::text_secondary(cx))
                    .child(if active {
                        "已开启 — 按 Esc 键退出"
                    } else {
                        "已关闭"
                    }),
            )
            .child(div().h(px(1.0)).bg(ui::border_light(cx)))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("自定义图片"),
            )
            .child(div().flex().gap(px(8.0)).items_center().child({
                div()
                    .flex_1()
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(ui::border_light(cx))
                    .bg(ui::bg_subtle(cx))
                    .text_size(px(12.0))
                    .child(draft_label)
            }))
            .child({
                let draft_for_select = Rc::clone(&draft);
                let error_for_select = Rc::clone(&error);
                let picker = Rc::clone(&self.picker);
                Button::new("anti-peeping-select-image")
                    .icon(icon!(folder_open))
                    .label("选择图片")
                    .on_click(move |_event, window, _cx| {
                        apply_picked_image(&draft_for_select, &error_for_select, picker());
                        window.refresh();
                    })
            })
            .child({
                let draft_for_clear = Rc::clone(&draft);
                let error_for_clear = Rc::clone(&error);
                Button::new("anti-peeping-clear-image")
                    .icon(icon!(trash_2))
                    .label("清除图片")
                    .on_click(move |_event, window, _cx| {
                        apply_clear(&draft_for_clear, &error_for_clear);
                        window.refresh();
                    })
            })
            .child({
                let draft_for_save = Rc::clone(&draft);
                let error_for_save = Rc::clone(&error);
                let ip_clone = Rc::clone(&ip);
                let paths_clone = self.paths.clone();
                div()
                    .id("save-image-config")
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(ui::success(cx))
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(gpui::white())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("保存设置")
                    .hover(|style| style.cursor_pointer())
                    .on_click(move |_event, window, _cx| {
                        match apply_save(&ip_clone, &error_for_save, &paths_clone, &draft_for_save)
                        {
                            Ok(()) => {}
                            Err(_) => {}
                        }
                        window.refresh();
                    })
            })
            .when_some(error_label, |container, msg| {
                container.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(ui::danger(cx))
                        .child(msg),
                )
            })
            .into_any_element()
    }

    fn on_close(&mut self) {
        *self.active.borrow_mut() = false;
    }

    fn on_close_with_app(&mut self, cx: &mut App) {
        *self.active.borrow_mut() = false;
        AntiPeepingPlugin::close_overlays(cx, &self.overlay_windows);
    }
}

/// Fullscreen overlay view — renders black or custom image, closes on Escape.
struct AntiPeepingOverlay {
    image_path: Rc<RefCell<Option<String>>>,
    active: Rc<RefCell<bool>>,
    overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
    focus_handle: FocusHandle,
}

impl AntiPeepingOverlay {
    fn new(
        image_path: Rc<RefCell<Option<String>>>,
        active: Rc<RefCell<bool>>,
        overlay_windows: Rc<RefCell<Vec<AnyWindowHandle>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            image_path,
            active,
            overlay_windows,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for AntiPeepingOverlay {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let ip = self.image_path.borrow();
        let image_path = ip.clone();
        let focus_handle = self.focus_handle.clone();
        let active = Rc::clone(&self.active);
        let overlay_windows = Rc::clone(&self.overlay_windows);

        div()
            .size_full()
            .bg(gpui::black())
            .track_focus(&focus_handle)
            .on_key_down(move |event, _window, cx| {
                if event.keystroke.key == "escape" {
                    *active.borrow_mut() = false;
                    cx.stop_propagation();
                    let overlay_windows = Rc::clone(&overlay_windows);
                    cx.defer(move |cx| AntiPeepingPlugin::close_overlays(cx, &overlay_windows));
                }
            })
            .when_some(image_path, |this, path| {
                this.child(img(PathBuf::from(path)).size_full())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use qingqi_plugin::events::AppEventBus;
    use qingqi_plugin::plugin::PluginCx;

    fn build_plugin() -> AntiPeepingPlugin {
        AntiPeepingPlugin::new(AppPaths::for_test(format!(
            "/tmp/qingqi-anti-peeping-test-{}",
            std::process::id()
        )))
    }

    fn temp_dir(name: &str) -> (std::path::PathBuf, AppPaths) {
        let dir = std::env::temp_dir().join(format!(
            "qingqi-anti-peeping-test-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).ok();
        (dir.clone(), AppPaths::for_test(dir))
    }

    fn write_png(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(dir).join(name);
        let img: image::ImageBuffer<image::Rgba<u8>, _> =
            image::ImageBuffer::from_pixel(1, 1, image::Rgba([128, 64, 32, 255]));
        let mut buf: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&p, &buf).unwrap();
        p
    }

    fn write_config(paths: &AppPaths, value: &str) {
        let config_path = paths.config("anti-peeping.json");
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(
            &config_path,
            serde_json::json!({"image_path": value}).to_string(),
        )
        .unwrap();
    }

    #[test]
    fn load_returns_none_for_missing_null_empty_and_blank() {
        let (_, paths) = temp_dir("load-empty");

        // 1. Missing config file → None.
        assert_eq!(AntiPeepingPlugin::load_custom_image(&paths), None);

        // 2. null → None.
        write_config(&paths, "null");
        let config_path = paths.config("anti-peeping.json");
        std::fs::write(&config_path, r#"{"image_path": null}"#).unwrap();
        assert_eq!(AntiPeepingPlugin::load_custom_image(&paths), None);

        // 3. "" → None.
        write_config(&paths, "");
        assert_eq!(AntiPeepingPlugin::load_custom_image(&paths), None);

        // 4.空白 → None.
        write_config(&paths, "   ");
        assert_eq!(AntiPeepingPlugin::load_custom_image(&paths), None);

        // 5. 平文空白文件 → None.
        std::fs::write(&config_path, "   \n").unwrap();
        assert_eq!(AntiPeepingPlugin::load_custom_image(&paths), None);

        // 6. Valid value round-trips.
        let valid_path = write_png(std::env::temp_dir().as_path(), "roundtrip.png");
        write_config(&paths, valid_path.to_str().unwrap());
        assert_eq!(
            AntiPeepingPlugin::load_custom_image(&paths),
            Some(valid_path.to_str().unwrap().to_string())
        );
    }

    #[test]
    fn save_valid_image_then_reload_round_trip() {
        let (dir, paths) = temp_dir("save-valid");
        let png = write_png(&dir, "valid.png");
        let png_str = png.to_str().unwrap();

        // 保存应在成功时返回 Ok，同时写入配置。
        AntiPeepingPlugin::save_custom_image(&paths, Some(png_str)).unwrap();

        // 加载回读的结果应一致。
        assert_eq!(
            AntiPeepingPlugin::load_custom_image(&paths),
            Some(png_str.to_string())
        );

        // 磁盘上的 JSON 包含正确的 UTF-8 路径字符串，非空值。
        let raw = std::fs::read_to_string(paths.config("anti-peeping.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["image_path"], png_str);
    }

    #[test]
    fn save_rejects_invalid_inputs_but_preserves_old_config() {
        let (dir, paths) = temp_dir("save-invalid");

        // 初始保存一张合法图片。
        let png = write_png(&dir, "original.png");
        let png_str = png.to_str().unwrap();
        AntiPeepingPlugin::save_custom_image(&paths, Some(png_str)).unwrap();

        // 1. 目录路径应报错，且不能改写现有配置。
        let err = AntiPeepingPlugin::save_custom_image(&paths, Some(&dir.to_str().unwrap()))
            .expect_err("目录路径应该失败");
        assert!(
            err.to_string().contains("不是文件") || err.to_string().contains("是文件"),
            "未提示 not-file: {err}"
        );

        // 2. 无效内容（被篡改成伪装的扩展名）应该报错。
        let fake = dir.join("fake.png");
        std::fs::write(&fake, b"not a real image").unwrap();
        let _ = AntiPeepingPlugin::save_custom_image(&paths, Some(fake.to_str().unwrap()))
            .expect_err("伪装图片应失败");

        // 3. 不可读路径应报错。
        let not_exist = dir.join("nope.png").to_str().unwrap().to_string();
        let _ = AntiPeepingPlugin::save_custom_image(&paths, Some(&not_exist))
            .expect_err("不存在的文件应失败");

        // 4. 上述全部失败后，初始合法配置应仍被保留。
        assert_eq!(
            AntiPeepingPlugin::load_custom_image(&paths),
            Some(png_str.to_string())
        );
    }

    #[test]
    fn save_to_unwritable_dir_returns_err() {
        // 指向一个不可写的父路径，强制 create_dir_all 失败。
        // 用存在的文件路径当“目录”来模拟该错误。
        let (dir, _) = temp_dir("save-unwritable");
        let blocker = dir.join("blocker_file");
        std::fs::write(&blocker, b"occupied").unwrap();
        // 以文件路径作为 data_dir，以便 config("anti-peeping.json") 落在不可写的子路径上。
        let blocked_paths = AppPaths::for_test(&blocker);

        // 保存应该失败，因为我们无法在其下创建 config/ 目录。
        let png = write_png(&dir, "ok.png");
        let result =
            AntiPeepingPlugin::save_custom_image(&blocked_paths, Some(png.to_str().unwrap()));
        assert!(result.is_err(), "不可写位置保存应报错");
    }

    /// 在测试中直接构造 AntiPeepingView，不使用 Plugin。
    fn build_direct_view(
        paths: AppPaths,
        picker: Rc<dyn Fn() -> Option<PathBuf>>,
    ) -> (AntiPeepingView, Rc<RefCell<Option<String>>>) {
        let image_path = Rc::new(RefCell::new(None));
        let view = AntiPeepingView {
            active: Rc::new(RefCell::new(false)),
            image_path: Rc::clone(&image_path),
            overlay_windows: Rc::new(RefCell::new(Vec::new())),
            paths,
            draft_path: Rc::new(RefCell::new(String::new())),
            draft_error: Rc::new(RefCell::new(None)),
            picker,
        };
        (view, image_path)
    }

    #[test]
    fn picker_select_invalid_then_pick_valid_updates_draft() {
        let (dir, paths) = temp_dir("pick-invalid-then-valid");
        let pick_result = Rc::new(RefCell::new(None::<PathBuf>));
        let picker = {
            let pick_result = Rc::clone(&pick_result);
            Rc::new(move || pick_result.borrow().clone())
        };
        let (view, _ip) = build_direct_view(paths, picker);

        // 1. 选择非法输入（目录）：应写入错误，不改动草稿。
        *pick_result.borrow_mut() = Some(dir.clone());
        view.apply_pick_from_injected_input();
        assert!(view.draft_error.borrow().is_some(), "目录路径应触发错误");
        assert!(
            view.draft_path.borrow().is_empty(),
            "选择失败时草稿应保持未变"
        );

        // 2. 替换为合法图片：应清除错误并写入草稿。
        let png = write_png(&dir, "late-ok.png");
        *pick_result.borrow_mut() = Some(png.clone());
        view.apply_pick_from_injected_input();
        assert!(view.draft_error.borrow().is_none());
        assert_eq!(view.draft_path.borrow().as_str(), png.to_str().unwrap());
    }

    #[test]
    fn picker_cancel_keeps_draft_then_clear_then_save_writes_none() {
        let (dir, paths) = temp_dir("cancel-clear-save");
        let png = write_png(&dir, "will-clear.png");
        let png_str = png.to_str().unwrap().to_string();

        let pick_result = Rc::new(RefCell::new(Some(png)));
        let picker = {
            let pick_result = Rc::clone(&pick_result);
            Rc::new(move || pick_result.borrow().clone())
        };
        let (view, ip) = build_direct_view(paths.clone(), picker);

        // 选择合法图片 → 草稿写入。
        view.apply_pick_from_injected_input();
        assert_eq!(*view.draft_path.borrow(), png_str);

        // 下次选择返回 None（模拟取消）不应改动草稿。
        *pick_result.borrow_mut() = None;
        view.apply_pick_from_injected_input();
        assert_eq!(*view.draft_path.borrow(), png_str, "取消选择不应改动草稿");

        // 清除草稿（只改草稿，不影响已保存配置）。
        view.apply_clear_click();
        assert!(view.draft_path.borrow().is_empty());

        // 保存空草稿 → 运行时 image_path 置为 None，并写入 null。
        apply_save(&ip, &view.draft_error, &view.paths, &view.draft_path).unwrap();
        assert!(ip.borrow().is_none());
        assert!(view.draft_error.borrow().is_none());

        // 重启后读取：不应把 null/空值解释为 Some("")。
        assert_eq!(
            AntiPeepingPlugin::load_custom_image(&paths),
            None,
            "空值持久化时应返回 None"
        );
    }

    #[gpui::test]
    fn ui_select_updates_draft_label(cx: &mut TestAppContext) {
        // 注入临时 png，通过 view 的模拟 click 方法驱动草稿变更。
        let png_dir =
            std::env::temp_dir().join(format!("qingqi-anti-peeping-ui-{}", std::process::id()));
        std::fs::create_dir_all(&png_dir).ok();
        let png = write_png(&png_dir, "ui.png");
        let png_str = png.to_str().unwrap().to_string();

        let paths = AppPaths::for_test(&png_dir);
        let pick_result = Rc::new(RefCell::new(None::<PathBuf>));
        let picker = {
            let pick_result = Rc::clone(&pick_result);
            Rc::new(move || pick_result.borrow().clone())
        };

        let (view, _) = cx.update(|_cx| build_direct_view(paths, picker));

        // 确认初始状态为空。
        assert!(cx.read(|_| view.draft_path.borrow().is_empty()));

        // 模拟「点击选择图片」并验证草稿与错误状态。
        *pick_result.borrow_mut() = Some(png.clone());
        cx.update(|_cx| view.apply_pick_from_injected_input());
        assert_eq!(view.draft_path.borrow().as_str(), png_str);
        assert!(view.draft_error.borrow().is_none());

        // 模拟「点击清除图片」应重置草稿。
        cx.update(|_cx| view.apply_clear_click());
        assert!(view.draft_path.borrow().is_empty());

        let _ = std::fs::remove_dir_all(&png_dir);
    }

    #[gpui::test]
    fn esc_closes_all_overlays_and_clears_active(cx: &mut TestAppContext) {
        let mut plugin = build_plugin();

        let view = cx.update(|cx| {
            let events = AppEventBus::new();
            let mut plugin_cx = PluginCx::new(events, cx);
            plugin.open(&mut plugin_cx).unwrap()
        });

        let mut window_view = match view {
            PluginView::Window(w) => w,
            _ => panic!("expected window view"),
        };

        assert!(*plugin.active.borrow());
        assert!(
            !plugin.overlay_windows.borrow().is_empty(),
            "opening the plugin should create at least one overlay window"
        );

        cx.update(|cx| {
            window_view.on_close_with_app(cx);
        });

        assert!(*plugin.active.borrow() == false);
        assert!(plugin.overlay_windows.borrow().is_empty());

        // Double close must not panic (idempotent drain).
        cx.update(|cx| {
            window_view.on_close_with_app(cx);
        });
        assert!(plugin.overlay_windows.borrow().is_empty());
    }

    #[gpui::test]
    fn close_plugin_window_drains_overlays_and_clears_active(cx: &mut TestAppContext) {
        let mut plugin = build_plugin();

        let view = cx.update(|cx| {
            let events = AppEventBus::new();
            let mut plugin_cx = PluginCx::new(events, cx);
            plugin.open(&mut plugin_cx).unwrap()
        });

        let mut window_view = match view {
            PluginView::Window(w) => w,
            _ => panic!("expected window view"),
        };

        assert!(*plugin.active.borrow());
        assert!(!plugin.overlay_windows.borrow().is_empty());

        cx.update(|cx| {
            window_view.on_close_with_app(cx);
        });

        assert!(*plugin.active.borrow() == false);
        assert!(plugin.overlay_windows.borrow().is_empty());
    }

    #[gpui::test]
    fn close_overlays_skips_failed_handle_and_drains_rest(cx: &mut TestAppContext) {
        let mut plugin = build_plugin();

        // Open genuine overlay windows so we have real handles.
        let real_handles = cx.update(|cx| {
            let events = AppEventBus::new();
            let mut plugin_cx = PluginCx::new(events, cx);
            let _ = plugin.open(&mut plugin_cx);
            plugin.overlay_windows.borrow().clone()
        });

        assert!(
            !real_handles.is_empty(),
            "expected at least one real overlay handle"
        );

        // Close the underlying overlay window(s) so their handles are stale,
        // then reintroduce them. Updates against released handles will fail,
        // but `close_overlays` must still drain the whole vector.
        for h in &real_handles {
            cx.update_window(*h, |_, window, _cx| {
                window.remove_window();
            })
            .ok();
        }
        *plugin.overlay_windows.borrow_mut() = real_handles;

        cx.update(|cx| {
            AntiPeepingPlugin::close_overlays(cx, &plugin.overlay_windows);
        });

        assert!(
            plugin.overlay_windows.borrow().is_empty(),
            "all handles must be drained even when individual updates fail"
        );
    }
}
