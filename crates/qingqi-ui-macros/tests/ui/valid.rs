use qingqi_ui_macros::lucide_path;

fn main() {
    assert_eq!(lucide_path!(search), "lucide/search.svg");
    assert_eq!(lucide_path!(settings_2), "lucide/settings-2.svg");
    assert_eq!(lucide_path!(wrap_text), "lucide/wrap-text.svg");
    assert_eq!(lucide_path!(r#move), "lucide/move.svg");
}
