     1|//! System tray: show window, prevent sleep, restart, quit.
     2|
     3|use std::{
     4|    process::Command,
     5|    sync::atomic::{AtomicBool, Ordering},
     6|    thread,
     7|    time::Duration,
     8|};
     9|
    10|#[cfg(any(target_os = "macos", target_os = "windows"))]
    11|use tray_icon::{
    12|    Icon, TrayIconBuilder,
    13|    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
    14|};
    15|
    16|use crate::power::PreventSleepMode;
    17|
    18|/// Tray menu actions.
    19|#[derive(Debug, Clone, Copy, PartialEq, Eq)]
    20|pub enum TrayAction {
    21|    Show,
    22|    SetPreventSleep(PreventSleepMode),
    23|    Restart,
    24|    Quit,
    25|}
    26|
    27|const MENU_SHOW: &str = "qingqi.tray.show";
    28|const MENU_SLEEP_DISABLED: &str = "qingqi.tray.sleep.disabled";
    29|const MENU_SLEEP_ALWAYS: &str = "qingqi.tray.sleep.always";
    30|const MENU_SLEEP_PLUGGED: &str = "qingqi.tray.sleep.plugged";
    31|const MENU_RESTART: &str = "qingqi.tray.restart";
    32|const MENU_QUIT: &str = "qingqi.tray.quit";
    33|
    34|#[cfg(any(target_os = "macos", target_os = "windows"))]
    35|static TRAY_INSTALLED: AtomicBool = AtomicBool::new(false);
    36|
    37|/// Install tray icon and menu. Call on the main thread after the event loop runs.
    38|pub fn install_tray(mode: PreventSleepMode) -> Result<(), String> {
    39|    #[cfg(any(target_os = "macos", target_os = "windows"))]
    40|    {
    41|        let menu = build_menu(mode)?;
    42|        let icon = default_icon()?;
    43|        let mut builder = TrayIconBuilder::new()
    44|            .with_menu(Box::new(menu))
    45|            .with_menu_on_left_click(false)
    46|            .with_tooltip("Qingqi");
    47|
    48|        #[cfg(target_os = "macos")]
    49|        {
    50|            builder = builder.with_icon_as_template(true);
    51|        }
    52|
    53|        let tray = builder
    54|            .with_icon(icon)
    55|            .build()
    56|            .map_err(|error| error.to_string())?;
    57|
    58|        // Drop the previous tray icon (replaces it in the system menu bar).
    59|        replace_tray(tray);
    60|        TRAY_INSTALLED.store(true, Ordering::SeqCst);
    61|        Ok(())
    62|    }
    63|
    64|    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    65|    {
    66|        let _ = mode;
    67|        Err(String::from("system tray not supported on this platform"))
    68|    }
    69|}
    70|
    71|/// Rebuild the tray menu with updated sleep mode check marks.
    72|pub fn rebuild_menu(mode: PreventSleepMode) -> Result<(), String> {
    73|    #[cfg(any(target_os = "macos", target_os = "windows"))]
    74|    {
    75|        let menu = build_menu(mode)?;
    76|        with_tray(|tray| {
    77|            tray.set_menu(Some(Box::new(menu)));
    78|        });
    79|        Ok(())
    80|    }
    81|    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    82|    {
    83|        let _ = mode;
    84|        Ok(())
    85|    }
    86|}
    87|
    88|/// Poll tray click and menu events. Returns pending actions.
    89|pub fn poll_actions() -> Vec<TrayAction> {
    90|    #[cfg(any(target_os = "macos", target_os = "windows"))]
    91|    {
    92|        use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
    93|
    94|        let mut actions = Vec::new();
    95|
    96|        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
    97|            if let TrayIconEvent::Click {
    98|                button: MouseButton::Left,
    99|                button_state: MouseButtonState::Up,
   100|                ..
   101|            } = event
   102|            {
   103|                actions.push(TrayAction::Show);
   104|            }
   105|        }
   106|
   107|        while let Ok(event) = MenuEvent::receiver().try_recv() {
   108|            let Some(action) = action_for_menu_id(event.id().as_ref()) else {
   109|                continue;
   110|            };
   111|            actions.push(action);
   112|        }
   113|
   114|        actions
   115|    }
   116|
   117|    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
   118|    {
   119|        Vec::new()
   120|    }
   121|}
   122|
   123|/// Launch a new process; caller exits the current one.
   124|pub fn relaunch() {
   125|    let Ok(exe) = std::env::current_exe() else {
   126|        tracing::warn!("restart failed: cannot resolve current executable");
   127|        return;
   128|    };
   129|
   130|    thread::spawn(move || {
   131|        thread::sleep(Duration::from_millis(280));
   132|        if let Err(error) = Command::new(&exe).spawn() {
   133|            tracing::warn!(error = %error, "restart spawn failed");
   134|        }
   135|    });
   136|}
   137|
   138|// ── Internals ──
   139|
   140|#[cfg(any(target_os = "macos", target_os = "windows"))]
   141|use tray_icon::TrayIcon;
   142|
   143|/// Stored tray icon. Only accessed from the main thread (GPUI event loop).
   144|#[cfg(any(target_os = "macos", target_os = "windows"))]
   145|static mut CURRENT_TRAY: Option<TrayIcon> = None;
   146|
   147|#[cfg(any(target_os = "macos", target_os = "windows"))]
   148|fn replace_tray(tray: TrayIcon) {
   149|    unsafe {
   150|        CURRENT_TRAY = Some(tray);
   151|    }
   152|}
   153|
   154|#[cfg(any(target_os = "macos", target_os = "windows"))]
   155|fn with_tray(f: impl FnOnce(&TrayIcon)) {
   156|    unsafe {
   157|        if let Some(ref tray) = CURRENT_TRAY {
   158|            f(tray);
   159|        }
   160|    }
   161|}
   162|
   163|#[cfg(any(target_os = "macos", target_os = "windows"))]
   164|fn build_menu(mode: PreventSleepMode) -> Result<Menu, String> {
   165|    let menu = Menu::new();
   166|
   167|    let show = MenuItem::with_id(MenuId::new(MENU_SHOW), "显示界面", true, None);
   168|    menu.append(&show).map_err(|error| error.to_string())?;
   169|    menu.append(&PredefinedMenuItem::separator())
   170|        .map_err(|error| error.to_string())?;
   171|
   172|    // ── Prevent Sleep submenu ──
   173|    let sleep_sub = Submenu::new("防止休眠", true);
   174|
   175|    let disabled = CheckMenuItem::with_id(
   176|        MenuId::new(MENU_SLEEP_DISABLED),
   177|        "不开启",
   178|        true,
   179|        mode == PreventSleepMode::Disabled,
   180|        None,
   181|    );
   182|    let always = CheckMenuItem::with_id(
   183|        MenuId::new(MENU_SLEEP_ALWAYS),
   184|        "始终开启",
   185|        true,
   186|        mode == PreventSleepMode::AlwaysOn,
   187|        None,
   188|    );
   189|    let plugged = CheckMenuItem::with_id(
   190|        MenuId::new(MENU_SLEEP_PLUGGED),
   191|        "仅接入电源开启",
   192|        true,
   193|        mode == PreventSleepMode::WhenPluggedIn,
   194|        None,
   195|    );
   196|
   197|    sleep_sub
   198|        .append(&disabled)
   199|        .map_err(|error| error.to_string())?;
   200|    sleep_sub
   201|        .append(&always)
   202|        .map_err(|error| error.to_string())?;
   203|    sleep_sub
   204|        .append(&plugged)
   205|        .map_err(|error| error.to_string())?;
   206|
   207|    menu.append(&sleep_sub).map_err(|error| error.to_string())?;
   208|    menu.append(&PredefinedMenuItem::separator())
   209|        .map_err(|error| error.to_string())?;
   210|
   211|    let restart = MenuItem::with_id(MenuId::new(MENU_RESTART), "重启", true, None);
   212|    let quit = MenuItem::with_id(MenuId::new(MENU_QUIT), "退出", true, None);
   213|
   214|    menu.append(&restart).map_err(|error| error.to_string())?;
   215|    menu.append(&PredefinedMenuItem::separator())
   216|        .map_err(|error| error.to_string())?;
   217|    menu.append(&quit).map_err(|error| error.to_string())?;
   218|    Ok(menu)
   219|}
   220|
   221|#[cfg(any(target_os = "macos", target_os = "windows"))]
   222|fn default_icon() -> Result<Icon, String> {
   223|    if let Some(icon) = load_tray_svg_icon() {
   224|        return Ok(icon);
   225|    }
   226|
   227|    const SIZE: u32 = 22;
   228|    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
   229|
   230|    fn right_edge(px: f32, py: f32) -> f32 {
   231|        let row = py as i32;
   232|        let b: f32 = match row {
   233|            0 => 10.5,
   234|            1 => 9.5,
   235|            2 => 8.5,
   236|            3 => 7.5,
   237|            4..=12 => 5.5,
   238|            13 => 7.5,
   239|            14 => 6.5,
   240|            15 => 5.5,
   241|            16 => 4.5,
   242|            17 => 3.5,
   243|            18 => 3.5,
   244|            19 => 2.5,
   245|            _ => 0.0,
   246|        };
   247|        let f: f32 = match row {
   248|            16 => 10.5,
   249|            17 => 10.5,
   250|            18 => 9.5,
   251|            _ => b,
   252|        };
   253|        f.max(b) - px
   254|    }
   255|
   256|    fn left_edge(px: f32, py: f32) -> f32 {
   257|        let row = py as i32;
   258|        let b: f32 = match row {
   259|            0 => 10.5,
   260|            1 => 11.5,
   261|            2 => 12.5,
   262|            3 => 13.5,
   263|            4..=12 => 15.5,
   264|            13 => 13.5,
   265|            14 => 14.5,
   266|            15 => 15.5,
   267|            16 => 16.5,
   268|            17 => 17.5,
   269|            18 => 17.5,
   270|            19 => 18.5,
   271|            _ => 0.0,
   272|        };
   273|        let f: f32 = match row {
   274|            16 => 10.5,
   275|            17 => 10.5,
   276|            18 => 11.5,
   277|            _ => b,
   278|        };
   279|        px - f.min(b)
   280|    }
   281|
   282|    for y in 0..SIZE {
   283|        for x in 0..SIZE {
   284|            let px = x as f32 + 0.5;
   285|            let py = y as f32 + 0.5;
   286|            let d = left_edge(px, py).min(right_edge(px, py));
   287|            let alpha = (1.0 - d.max(0.0).min(1.0)).max(0.0).min(1.0);
   288|            let idx = ((y * SIZE + x) * 4) as usize;
   289|            rgba[idx] = 255;
   290|            rgba[idx + 1] = 255;
   291|            rgba[idx + 2] = 255;
   292|            rgba[idx + 3] = (alpha * 255.0) as u8;
   293|        }
   294|    }
   295|
   296|    Icon::from_rgba(rgba, SIZE, SIZE).map_err(|error| error.to_string())
   297|}
   298|
   299|#[cfg(any(target_os = "macos", target_os = "windows"))]
   300|fn load_tray_svg_icon() -> Option<Icon> {
   301|    // macOS 菜单栏标准逻辑尺寸为 22pt，使用 2x 位图保证 Retina 清晰
   302|    const SIZE: u32 = 44;
   303|    let path =
   304|        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../qingqi/assets/tray-icon.svg");
   305|    let rgba = crate::svg_icon::rasterize_path(path.as_path(), SIZE).ok()?;
   306|    icon_from_rgba_template(rgba, SIZE, SIZE).ok()
   307|}
   308|
   309|/// 转为模板图标：黑色剪影 + alpha（macOS 会根据深浅色菜单栏自动反转）。
   310|#[cfg(any(target_os = "macos", target_os = "windows"))]
   311|fn icon_from_rgba_template(rgba: Vec<u8>, width: u32, height: u32) -> Result<Icon, String> {
   312|    let mut out = rgba;
   313|    for chunk in out.chunks_exact_mut(4) {
   314|        let alpha = chunk[3];
   315|        chunk[0] = 0;
   316|        chunk[1] = 0;
   317|        chunk[2] = 0;
   318|        chunk[3] = alpha;
   319|    }
   320|    Icon::from_rgba(out, width, height).map_err(|error| error.to_string())
   321|}
   322|
   323|fn action_for_menu_id(id: &str) -> Option<TrayAction> {
   324|    match id {
   325|        MENU_SHOW => Some(TrayAction::Show),
   326|        MENU_SLEEP_DISABLED => Some(TrayAction::SetPreventSleep(PreventSleepMode::Disabled)),
   327|        MENU_SLEEP_ALWAYS => Some(TrayAction::SetPreventSleep(PreventSleepMode::AlwaysOn)),
   328|        MENU_SLEEP_PLUGGED => Some(TrayAction::SetPreventSleep(PreventSleepMode::WhenPluggedIn)),
   329|        MENU_RESTART => Some(TrayAction::Restart),
   330|        MENU_QUIT => Some(TrayAction::Quit),
   331|        _ => None,
   332|    }
   333|}
   334|