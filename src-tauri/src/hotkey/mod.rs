use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 默认快捷键
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

/// 获取当前前台应用的 bundle ID（macOS）
/// 使用 lsappinfo 命令，无需 Automation 权限
#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id() -> Option<String> {
    crate::clipboard::get_frontmost_app_bundle_id()
}

/// 将窗口定位到屏幕底部，宽度铺满屏幕
fn position_at_bottom(window: &tauri::WebviewWindow) {
    // 优先按光标所在屏幕定位（多显示器：在哪块屏操作就在哪块弹出）。
    // 隐藏窗口的 current_monitor() 只反映窗口上次所在屏，无法跟随光标；
    // 故取光标物理坐标，逐个比对显示器物理边界找出命中屏，失败时回退 current_monitor()。
    let mut matches: Vec<tauri::Monitor> = Vec::new();
    if let Ok(pos) = window.cursor_position() {
        if let Ok(monitors) = window.available_monitors() {
            for m in monitors {
                let mp = m.position();
                let ms = m.size();
                let (x0, y0) = (mp.x as f64, mp.y as f64);
                let (x1, y1) = (x0 + ms.width as f64, y0 + ms.height as f64);
                if pos.x >= x0 && pos.x < x1 && pos.y >= y0 && pos.y < y1 {
                    matches.push(m);
                }
            }
        }
    }
    // 镜像（复制）屏模式下多块显示器坐标重叠会同时命中；
    // 镜像组的真实逻辑坐标空间以主屏为准，此时优先取主屏，避免用错尺寸/缩放导致面板悬在半空
    let multi_matched = matches.len() > 1;
    let monitor = if multi_matched {
        let primary = window.primary_monitor().ok().flatten();
        primary
            .and_then(|p| {
                matches
                    .iter()
                    .find(|m| m.position() == p.position() && m.size() == p.size())
                    .cloned()
            })
            .or_else(|| matches.first().cloned())
    } else {
        matches.into_iter().next()
    }
    .or_else(|| window.current_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        // 用工作区域而非全屏尺寸：自动扣除 Windows 任务栏 / macOS Dock，避免面板底部被遮挡
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();

        let screen_w = work_area.size.width as f64 / scale;
        let screen_h = work_area.size.height as f64 / scale;
        let offset_x = work_area.position.x as f64 / scale;
        let offset_y = work_area.position.y as f64 / scale;

        let panel_height = 400.0;
        let dock_margin = 5.0;

        // 设置窗口大小为工作区宽度 × 面板高度
        let _ = window.set_size(tauri::LogicalSize::new(screen_w, panel_height));

        // 定位到工作区最底部（留一点间距）
        let x = offset_x;
        let y = offset_y + screen_h - panel_height - dock_margin;

        let _ = window.set_position(tauri::LogicalPosition::new(x, y));

        // 选屏与定位结果日志：含工作区原始数据与计算结果，供排查镜像/多屏定位错位
        crate::logger::log(&format!(
            "panel: monitor={:?} multi_matched={} scale={} wa_pos=({},{}) wa_size=({},{}) -> pos=({:.0},{:.0}) size=({:.0},{})",
            monitor.name(), multi_matched, scale,
            work_area.position.x, work_area.position.y,
            work_area.size.width, work_area.size.height,
            x, y, screen_w, panel_height
        ));
        // 实际生效位置回读：若与目标不一致可直接定位是设置被系统修正还是计算错误
        if let (Ok(p), Ok(s)) = (window.outer_position(), window.outer_size()) {
            crate::logger::log(&format!(
                "panel: actual pos=({},{}) size=({},{}) [physical]",
                p.x, p.y, s.width, s.height
            ));
        }
    }
}

/// 注册全局快捷键来 toggle 窗口
pub fn register_shortcuts(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut: Shortcut = DEFAULT_SHORTCUT.parse()?;

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            crate::logger::log("hotkey: dismiss panel");
                            // 不直接 hide：通知前端播完滑出动画后自行隐藏
                            let _ = window.emit("panel-dismiss", ());
                        } else {
                            // macOS: 记录当前前台应用，粘贴时恢复焦点
                            #[cfg(target_os = "macos")]
                            {
                                let front = get_frontmost_app_bundle_id();
                                crate::logger::log(&format!("hotkey: show panel, front={:?}", front));
                                if let Some(id) = front {
                                    let state = app.state::<crate::commands::AppState>();
                                    *state.previous_app.lock().unwrap() = Some(id);
                                }
                            }
                            // Windows: 记录当前前台窗口句柄，粘贴时恢复焦点
                            #[cfg(target_os = "windows")]
                            {
                                crate::logger::log("hotkey: show panel");
                                let hwnd = crate::clipboard::get_foreground_window_handle();
                                let state = app.state::<crate::commands::AppState>();
                                *state.previous_window_hwnd.lock().unwrap() = hwnd;
                            }
                            position_at_bottom(&window);
                            let _ = window.show();
                            let _ = window.set_focus();
                            // 部分应用（如 Chrome）不让出焦点时 tauri://focus 不会触发，
                            // 主动通知前端播入场动画，避免面板停在 translateY(100%) 隐形态
                            let _ = window.emit("panel-shown", ());
                        }
                    }
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(shortcut)?;
    Ok(())
}

/// 重新注册快捷键（先取消所有，再注册新的）
pub fn re_register_shortcut(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    let shortcut: Shortcut = shortcut_str.parse().map_err(|e| format!("{:?}", e))?;

    // 取消所有已注册快捷键
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;

    // 注册新快捷键
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| e.to_string())?;

    Ok(())
}
