use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use gpui::{
    AnyWindowHandle, App, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::tree::TreeState;
use gpui_component::{
    IconName, Sizable, Size,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};

use crate::code_gen::CodeLanguage;
use crate::service::{
    ApiEnvironment, ApiGroup, ApiRequest, ApiResponse, ApiService, AuthType, BodyMode, EditorTab,
    HttpHistory, HttpMethod, ResponseTab,
};
use gpui_component::{input::InputState, theme::Theme};
use qingqi_ui::ui::glass;

use qingqi_plugin::plugin_spec::PluginAccent;

pub mod collection;
pub mod components;
pub mod editor;
pub mod env;
pub mod request;
pub mod response;
pub mod types;

pub(crate) const STACK_BREAKPOINT_PX: f32 = 980.0;
#[allow(unused)]
pub(crate) const ACCENT: PluginAccent = PluginAccent::Blue;

pub struct ApiDebuggerView {
    pub(crate) service: Arc<ApiService>,
    pub(crate) groups: Vec<ApiGroup>,
    pub(crate) environments: Vec<ApiEnvironment>,
    pub(crate) selected_request: usize,
    pub(crate) selected_scenario: Option<usize>,
    pub(crate) selected_environment: usize,
    pub(crate) editor_tab: EditorTab,
    pub(crate) response_tab: ResponseTab,
    pub(crate) response_code_lang: CodeLanguage,
    pub(crate) history_entries: Vec<HttpHistory>,
    pub(crate) body_mode: BodyMode,
    pub(crate) auth_type: AuthType,
    pub(crate) show_method_popover: bool,
    pub(crate) show_env_popover: bool,
    pub(crate) show_collection_menu: bool,
    pub(crate) collection_menu_title: String,
    pub(crate) collection_menu_position: Option<(f32, f32)>,
    pub(crate) collection_menu_node_id: String,
    pub(crate) collection_menu_kind: Option<components::collection_tree::MenuKind>,
    pub(crate) show_curl_import: bool,
    pub(crate) curl_import_input: Entity<InputState>,
    pub(crate) rename_node_id: String,
    pub(crate) renaming_node_id: String,
    pub(crate) rename_inline_input: Entity<InputState>,
    pub(crate) show_rename: bool,
    pub(crate) rename_input: Entity<InputState>,
    pub(crate) env_editor_window: Option<AnyWindowHandle>,
    pub(crate) path_input: Entity<InputState>,
    pub(crate) params_kv: types::KvEditor,
    pub(crate) path_kv: types::KvEditor,
    pub(crate) body_input: Entity<InputState>,
    pub(crate) headers_kv: types::KvEditor,
    pub(crate) cookies_kv: types::KvEditor,
    pub(crate) auth_bearer_input: Entity<InputState>,
    pub(crate) auth_basic_user_input: Entity<InputState>,
    pub(crate) auth_basic_pass_input: Entity<InputState>,
    pub(crate) auth_apikey_name_input: Entity<InputState>,
    pub(crate) auth_apikey_value_input: Entity<InputState>,
    pub(crate) auth_apikey_in_query: bool,
    pub(crate) pre_ops_input: Entity<InputState>,
    pub(crate) post_ops_input: Entity<InputState>,
    pub(crate) env_name_input: Entity<InputState>,
    pub(crate) env_base_url_input: Entity<InputState>,
    pub(crate) env_variables_input: Entity<InputState>,
    pub(crate) env_headers_input: Entity<InputState>,
    pub(crate) response: ApiResponse,
    pub(crate) notice: String,
    pub(crate) last_revision: u64,
    pub(crate) tree_state: Entity<TreeState>,
    pub(crate) collapsed_nodes: RefCell<HashSet<String>>,
}

impl ApiDebuggerView {
    pub fn new(service: Arc<ApiService>, window: &mut Window, cx: &mut App) -> Self {
        let workspace_result = service.load_workspace();
        let (groups, environments, notice) = match workspace_result {
            Ok(workspace) => {
                if workspace.groups.is_empty()
                    || workspace.groups.iter().all(|g| g.requests.is_empty())
                {
                    let empty_request = ApiRequest {
                        node_id: String::new(),
                        title: String::from("新请求"),
                        method: HttpMethod::Get,
                        path: String::from("/"),
                        params: Vec::new(),
                        path_rows: Vec::new(),
                        body: String::new(),
                        body_mode: BodyMode::None,
                        headers: Vec::new(),
                        cookies: Vec::new(),
                        auth: Vec::new(),
                        pre_ops: String::new(),
                        post_ops: String::new(),
                        scenarios: Vec::new(),
                    };
                    (
                        vec![ApiGroup {
                            id: None,
                            name: String::from("集合"),
                            folders: Vec::new(),
                            requests: vec![empty_request],
                        }],
                        workspace.environments,
                        String::from("集合为空，点击 + 创建第一个请求"),
                    )
                } else {
                    (
                        workspace.groups,
                        workspace.environments,
                        String::from("已加载 API 调试器"),
                    )
                }
            }
            Err(error) => {
                let empty_request = ApiRequest {
                    node_id: String::new(),
                    title: String::from("新请求"),
                    method: HttpMethod::Get,
                    path: String::from("/"),
                    params: Vec::new(),
                    path_rows: Vec::new(),
                    body: String::new(),
                    body_mode: BodyMode::None,
                    headers: Vec::new(),
                    cookies: Vec::new(),
                    auth: Vec::new(),
                    pre_ops: String::new(),
                    post_ops: String::new(),
                    scenarios: Vec::new(),
                };
                (
                    vec![ApiGroup {
                        id: None,
                        name: String::from("集合"),
                        folders: Vec::new(),
                        requests: vec![empty_request],
                    }],
                    service.list_environments_ui(),
                    format!("工作区加载失败: {error}"),
                )
            }
        };
        let selected_request = 0usize;
        let selected_scenario = types::request_at(&groups, selected_request)
            .and_then(|request| (!request.scenarios.is_empty()).then_some(0));

        let base_request = types::request_at(&groups, selected_request)
            .expect("api request should exist")
            .clone();
        let request = selected_scenario
            .and_then(|scenario_index| {
                base_request
                    .scenarios
                    .get(scenario_index)
                    .and_then(|scenario| scenario.request.as_deref())
            })
            .cloned()
            .unwrap_or_else(|| base_request.clone());
        let environment = environments
            .first()
            .cloned()
            .unwrap_or_else(|| ApiEnvironment {
                name: String::from("默认环境"),
                badge: String::from("默"),
                color: 0x338855,
                base_url: String::from("http://127.0.0.1:8000"),
                variables: Vec::new(),
                headers: Vec::new(),
            });

        let (
            init_path,
            init_params,
            init_path_rows,
            init_body,
            init_headers,
            init_cookies,
            init_auth,
            init_pre_ops,
            init_post_ops,
            init_editor_tab,
        ) = (
            request.path.clone(),
            types::format_rows(&request.params),
            types::format_rows(&request.path_rows),
            request.body.clone(),
            types::format_rows(&request.headers),
            types::format_rows(&request.cookies),
            types::format_rows(&request.auth),
            request.pre_ops.clone(),
            request.post_ops.clone(),
            EditorTab::Params,
        );

        let rev = service.revision();
        let init_auth_form = types::derive_auth_form(&types::parse_rows(&init_auth));

        let tree_state = {
            let mut idx = 0usize;
            let items =
                components::collection_tree::build_tree_items(&groups, &mut idx, &HashSet::new());
            cx.new(|cx| TreeState::new(cx).items(items))
        };

        Self {
            service,
            groups,
            environments,
            selected_request,
            selected_scenario,
            selected_environment: 0,
            editor_tab: init_editor_tab,
            response_tab: ResponseTab::Body,
            response_code_lang: CodeLanguage::Curl,
            history_entries: Vec::new(),
            body_mode: BodyMode::from_db(&types::detect_body_mode(&init_body)),
            auth_type: init_auth_form.auth_type.unwrap_or(AuthType::None),
            show_method_popover: false,
            show_env_popover: false,
            show_collection_menu: false,
            collection_menu_title: String::from("集合"),
            collection_menu_position: None,
            collection_menu_node_id: String::new(),
            collection_menu_kind: None,
            show_curl_import: false,
            curl_import_input: types::multiline_input(window, cx, "", "粘贴 cURL 命令..."),
            show_rename: false,
            rename_input: types::single_input(window, cx, "", "输入新名称..."),
            rename_node_id: String::new(),
            renaming_node_id: String::new(),
            rename_inline_input: types::single_input(window, cx, "", "重命名..."),
            env_editor_window: None,
            path_input: types::single_input(window, cx, &init_path, "/api/v1/user/info"),
            params_kv: types::KvEditor::from_text(window, cx, &init_params),
            path_kv: types::KvEditor::from_text(window, cx, &init_path_rows),
            body_input: types::multiline_input(window, cx, &init_body, "{ }"),
            headers_kv: types::KvEditor::from_text(window, cx, &init_headers),
            cookies_kv: types::KvEditor::from_text(window, cx, &init_cookies),
            auth_bearer_input: types::single_input(window, cx, &init_auth_form.bearer, "Token"),
            auth_basic_user_input: types::single_input(
                window,
                cx,
                &init_auth_form.basic_user,
                "用户名",
            ),
            auth_basic_pass_input: types::single_input(
                window,
                cx,
                &init_auth_form.basic_pass,
                "密码",
            ),
            auth_apikey_name_input: types::single_input(
                window,
                cx,
                &init_auth_form.apikey_name,
                "Key（如 X-API-Key）",
            ),
            auth_apikey_value_input: types::single_input(
                window,
                cx,
                &init_auth_form.apikey_value,
                "Value",
            ),
            auth_apikey_in_query: init_auth_form.in_query,
            pre_ops_input: types::multiline_input(window, cx, &init_pre_ops, "Pre-ops"),
            post_ops_input: types::multiline_input(window, cx, &init_post_ops, "Post-ops"),
            env_name_input: types::single_input(window, cx, &environment.name, "环境名称"),
            env_base_url_input: types::single_input(
                window,
                cx,
                &environment.base_url,
                "http://localhost:8080",
            ),
            env_variables_input: types::multiline_input(
                window,
                cx,
                &types::format_rows(&environment.variables),
                "KEY=VALUE",
            ),
            env_headers_input: types::multiline_input(
                window,
                cx,
                &types::format_rows(&environment.headers),
                "KEY=VALUE",
            ),
            response: types::sample_response(),
            notice,
            last_revision: rev,
            tree_state,
            collapsed_nodes: RefCell::new(HashSet::new()),
        }
    }
}

impl Render for ApiDebuggerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_service_updates(cx);

        let stacked = window.bounds().size.width < px(STACK_BREAKPOINT_PX);

        let entity = cx.entity();
        let show_method_popover = self.show_method_popover;
        let editor_tab = self.editor_tab;
        let body_mode = self.body_mode;
        let auth_type = self.auth_type;
        let response_tab = self.response_tab;
        let show_collection_menu = self.show_collection_menu;
        let show_curl_import = self.show_curl_import;
        let collection_menu_title = self.collection_menu_title.clone();
        let collection_menu_position = self.collection_menu_position;
        let collection_menu_node_id = self.collection_menu_node_id.clone();
        let collection_menu_kind = self.collection_menu_kind;
        let path_input = self.path_input.clone();
        let editor_text_input = self.text_editor_input(editor_tab);
        let editor_kv_rows = self
            .kv_editor(editor_tab)
            .map(|editor| editor.rows.clone())
            .unwrap_or_default();
        let editor_auth_form = self.auth_form_inputs();
        let curl_import_input = self.curl_import_input.clone();
        let show_rename = self.show_rename;
        let rename_input = self.rename_input.clone();
        let response = self.response.clone();
        let response_text = self.response_text();
        let response_history = self.history_entries.clone();
        let response_code_lang = self.response_code_lang;
        let notice = self.notice.clone();
        let current_request = self.selected_request().clone();
        let current_environment = self.selected_environment().clone();
        let in_flight = self.service.is_in_flight();
        let current_method = self.selected_request().method;

        let esc_view = entity.clone();

        let app: &App = cx;

        div()
            .relative()
            .size_full()
            .bg(glass::bg(cx))
            .rounded(px(12.0))
            .overflow_hidden()
            .font_family(".SystemUIFont")
            .text_color(Theme::global(cx).foreground)
            .on_key_down(move |event, _window, cx| {
                if event.keystroke.key == "escape" {
                    esc_view.update(cx, |view, _cx| {
                        view.show_collection_menu = false;
                        view.show_curl_import = false;
                    });
                }
            })
            .child(
                div()
                    .size_full()
                    .pt(px(0.0))
                    .pl(px(8.0))
                    .pr(px(8.0))
                    .pb(px(6.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .gap(px(10.0))
                            .when(stacked, |layout| layout.flex_col())
                            .when(!stacked, |layout| layout.flex_row())
                            .child(
                                div()
                                    .w(px(260.0))
                                    .min_h(px(0.0))
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .h(px(32.0))
                                            .flex()
                                            .items_center()
                                            .pl(px(86.0))
                                            .pr_2()
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child("API 调试"),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                Button::new("api-sidebar-new")
                                                    .ghost()
                                                    .icon(IconName::Plus)
                                                    .with_size(Size::XSmall)
                                                    .dropdown_menu({
                                                        let view = entity.clone();
                                                        move |menu, _window, _| {
                                                            let v1 = view.clone();
                                                            let v2 = view.clone();
                                                            let v3 = view.clone();
                                                            let v4 = view.clone();
                                                            let v5 = view.clone();
                                                            let v6 = view.clone();
                                                            menu
                                                                .item(PopupMenuItem::new("新建文件夹")
                                                                    .on_click(move |_, _, cx| {
                                                                        v1.update(cx, |view, _cx| {
                                                                            view.collection_menu_node_id = String::new();
                                                                            view.create_new_folder();
                                                                        });
                                                                    }))
                                                                .item(PopupMenuItem::new("新建接口")
                                                                    .on_click(move |_, _, cx| {
                                                                        v2.update(cx, |view, _cx| {
                                                                            view.collection_menu_node_id = String::new();
                                                                            view.create_new_endpoint();
                                                                        });
                                                                    }))
                                                                .item(PopupMenuItem::new("导入 cURL")
                                                                    .on_click(move |_, _, cx| {
                                                                        v3.update(cx, |view, _cx| {
                                                                            view.show_curl_import = true;
                                                                        });
                                                                    }))
                                                                .item(PopupMenuItem::new("导入 OpenAPI")
                                                                    .on_click(move |_, _, cx| {
                                                                        v4.update(cx, |view, _cx| view.import_openapi_file());
                                                                    }))
                                                                .item(PopupMenuItem::new("导入 Postman")
                                                                    .on_click(move |_, _, cx| {
                                                                        v5.update(cx, |view, _cx| view.import_postman_file());
                                                                    }))
                                                                .item(PopupMenuItem::new("导出为 OpenAPI")
                                                                    .on_click(move |_, _, cx| {
                                                                        v6.update(cx, |view, _cx| view.export_openapi());
                                                                    }))
                                                        }
                                                    }),
                                            ),
                                    )
                                    .child(components::collection_tree::collection_tree(
                                        entity.clone(),
                                        self.tree_state.clone(),
                                        app,
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .border_1()
                                    .border_color(glass::divider(cx))
                                    .bg(glass::bg(cx))
                                    .rounded(px(10.0))
                                    .overflow_hidden()
                                    .flex()
                                    .flex_col()
                                    .child(components::action_bar::action_bar(
                                        entity.clone(),
                                        current_request.clone(),
                                        current_environment.clone(),
                                        self.environments.clone(),
                                        self.selected_environment,
                                        path_input,
                                        in_flight,
                                        app,
                                        current_method,
                                        show_method_popover,
                                        self.show_env_popover,
                                    ))
                                    .child(
                                        components::shared::content_split(stacked)
                                            .child(components::editor_panel::editor_panel(
                                                entity.clone(),
                                                editor_tab,
                                                editor_text_input,
                                                editor_kv_rows,
                                                editor_auth_form,
                                                body_mode,
                                                auth_type,
                                                app,
                                            ))
                                            .child(components::response_panel::response_panel(
                                                entity.clone(),
                                                response_tab,
                                                response,
                                                response_text,
                                                response_history,
                                                response_code_lang,
                                                notice,
                                                app,
                                            )),
                                    ),
                            ),
                    ),
            )
            .child(if show_collection_menu {
                components::context_menu::context_menu_overlay(
                    entity.clone(),
                    collection_menu_title,
                    collection_menu_position,
                    collection_menu_node_id,
                    collection_menu_kind.unwrap_or(components::collection_tree::MenuKind::Folder),
                    app,
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if show_curl_import {
                components::dialogs::overlay_shell(
                    app,
                    "api-curl-import-backdrop",
                    {
                        let view = entity.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.show_curl_import = false);
                        }
                    },
                    components::dialogs::curl_import_dialog(entity.clone(), curl_import_input, app),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if show_rename {
                components::dialogs::overlay_shell(
                    app,
                    "api-rename-backdrop",
                    {
                        let view = entity.clone();
                        move |_, cx| {
                            view.update(cx, |view, _cx| view.show_rename = false);
                        }
                    },
                    components::dialogs::rename_dialog(entity.clone(), rename_input, app),
                )
                .into_any_element()
            } else {
                div().into_any_element()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::types::*;
    use crate::service::*;

    #[test]
    fn parse_rows_decodes_disabled_prefix() {
        let rows = parse_rows("Accept=application/json\n# X-Debug=1\nempty");
        assert_eq!(rows.len(), 3);

        assert!(rows[0].enabled);
        assert_eq!(rows[0].key, "Accept");
        assert_eq!(rows[0].value, "application/json");

        assert!(!rows[1].enabled);
        assert_eq!(rows[1].key, "X-Debug");
        assert_eq!(rows[1].value, "1");

        assert!(rows[2].enabled);
        assert_eq!(rows[2].key, "empty");
        assert_eq!(rows[2].value, "");
    }

    #[test]
    fn format_rows_encodes_disabled_prefix() {
        let rows = vec![
            KeyValueRow::new("a", "1"),
            KeyValueRow {
                enabled: false,
                key: "b".into(),
                value: "2".into(),
                value_type: String::new(),
                description: String::new(),
            },
        ];
        assert_eq!(format_rows(&rows), "a=1\n# b=2");
    }

    #[test]
    fn rows_text_roundtrip_preserves_enabled() {
        let original = vec![
            KeyValueRow::new("page", "1"),
            KeyValueRow {
                enabled: false,
                key: "limit".into(),
                value: "10".into(),
                value_type: String::new(),
                description: String::new(),
            },
            KeyValueRow::new("sort", "desc"),
        ];
        let restored = parse_rows(&format_rows(&original));
        assert_eq!(restored.len(), original.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert_eq!(a.enabled, b.enabled);
            assert_eq!(a.key, b.key);
            assert_eq!(a.value, b.value);
        }
    }

    #[test]
    fn rows_text_roundtrip_preserves_type_and_description() {
        let original = vec![KeyValueRow {
            enabled: true,
            key: "page".into(),
            value: "1".into(),
            value_type: "number".into(),
            description: "页码".into(),
        }];

        let text = format_rows(&original);
        assert_eq!(text, "page=1\tnumber\t页码");

        let restored = parse_rows(&text);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].value_type, "number");
        assert_eq!(restored[0].description, "页码");
    }

    #[test]
    fn value_with_hash_is_not_treated_as_disabled() {
        let rows = parse_rows("color=#fff");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].enabled);
        assert_eq!(rows[0].key, "color");
        assert_eq!(rows[0].value, "#fff");
    }

    #[test]
    fn content_type_extension_maps_known_types() {
        assert_eq!(
            content_type_extension("application/json; charset=utf-8"),
            "json"
        );
        assert_eq!(content_type_extension("image/png"), "png");
        assert_eq!(content_type_extension("text/html"), "html");
        assert_eq!(content_type_extension("application/octet-stream"), "txt");
        assert_eq!(content_type_extension(""), "txt");
    }

    #[test]
    fn binary_content_types_are_flagged() {
        assert!(is_binary_content_type("image/jpeg"));
        assert!(is_binary_content_type("application/pdf"));
        assert!(is_binary_content_type("video/mp4"));
        assert!(!is_binary_content_type("application/json"));
        assert!(!is_binary_content_type("text/plain"));
    }
}
