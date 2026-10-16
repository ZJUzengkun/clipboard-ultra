use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "macos")]
static PANEL_TRANSITION_ID: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "macos")]
const SHOW_DURATION_SECS: f64 = 0.16;
#[cfg(target_os = "macos")]
const HIDE_DURATION_SECS: f64 = 0.12;

/// 默认快捷键
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

/// 获取当前前台应用的 bundle ID（macOS）
/// 使用 lsappinfo 命令，无需 Automation 权限
#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id() -> Option<String> {
    crate::clipboard::get_frontmost_app_bundle_id()
}

/// 将窗口定位到屏幕底部，宽度铺满屏幕
#[cfg(not(target_os = "macos"))]
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

        // work_area 和 cursor_position 都是物理坐标。这里必须继续使用物理坐标设置窗口：
        // 若先换算成目标屏的逻辑坐标，Tauri 在窗口仍位于旧屏时可能按旧屏缩放比
        // 再转回物理坐标，跨不同缩放比显示器时就会导致面板悬在屏幕中间。
        let panel_height = (400.0 * scale).round() as u32;
        let dock_margin = (5.0 * scale).round() as i32;
        let x = work_area.position.x;
        let y =
            work_area.position.y + work_area.size.height as i32 - panel_height as i32 - dock_margin;

        // 设置窗口大小为目标屏工作区宽度 × 400 逻辑像素高，并贴近工作区底部。
        let _ = window.set_size(tauri::PhysicalSize::new(work_area.size.width, panel_height));
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));

        // 选屏与定位结果日志：含工作区原始数据与计算结果，供排查镜像/多屏定位错位
        crate::logger::log(&format!(
            "panel: monitor={:?} multi_matched={} scale={} wa_pos=({},{}) wa_size=({},{}) -> pos=({},{}) size=({},{}) [physical]",
            monitor.name(), multi_matched, scale,
            work_area.position.x, work_area.position.y,
            work_area.size.width, work_area.size.height,
            x, y, work_area.size.width, panel_height
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

/// macOS 使用 AppKit 在主线程中一次性设置 frame 并显示窗口。
/// NSWindow frame 与 NSScreen visibleFrame 使用同一套点坐标（左下角原点），
/// 避免 Tauri 物理/逻辑坐标转换和隐藏窗口异步恢复 frame 的竞态。
#[cfg(target_os = "macos")]
pub fn show_panel(window: &tauri::WebviewWindow) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAnimatablePropertyContainer, NSAnimationContext, NSApplication, NSEvent, NSScreen,
        NSWindow,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    // 让上一轮延迟 hide 失效，避免快速再次呼出后被旧任务误关。
    PANEL_TRANSITION_ID.fetch_add(1, Ordering::SeqCst);
    let target_window = window.clone();
    let emit_window = window.clone();
    let result = window.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            crate::logger::log("panel: AppKit show called outside main thread");
            return;
        };
        let mouse = NSEvent::mouseLocation();
        let screens = NSScreen::screens(mtm);
        let mut selected = None;
        for index in 0..screens.count() {
            let screen = screens.objectAtIndex(index);
            let frame = screen.frame();
            if mouse.x >= frame.origin.x
                && mouse.x < frame.origin.x + frame.size.width
                && mouse.y >= frame.origin.y
                && mouse.y < frame.origin.y + frame.size.height
            {
                selected = Some(screen);
                break;
            }
        }
        let screen = selected.or_else(|| NSScreen::mainScreen(mtm));
        let Some(screen) = screen else {
            crate::logger::log("panel: AppKit found no screen");
            return;
        };
        let visible = screen.visibleFrame();
        let height = 400.0_f64.min(visible.size.height);
        let frame = NSRect::new(
            NSPoint::new(visible.origin.x, visible.origin.y + 5.0),
            NSSize::new(visible.size.width, height),
        );

        let Ok(raw_window) = target_window.ns_window() else {
            crate::logger::log("panel: failed to get NSWindow");
            return;
        };
        let ns_window = unsafe { &*raw_window.cast::<NSWindow>() };
        // 每次都从当前屏幕的下边界外开始，由 AppKit 动画到最终 frame。
        // 动画移动的是原生窗口，而不是 WebView 内容，因此不会出现透明窗口先占位、
        // 内容再叠化的观感；终点仍是同一套 AppKit point 坐标。
        let start_frame = NSRect::new(
            NSPoint::new(visible.origin.x, visible.origin.y - height),
            NSSize::new(visible.size.width, height),
        );
        ns_window.setFrame_display(start_frame, false);
        // makeKeyAndOrderFront 只负责窗口层级；应用不在前台时不会保证获得键盘焦点。
        // 全局快捷键呼出必须显式激活应用，后续失焦通知才能形成完整闭环。
        let application = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        application.activateIgnoringOtherApps(true);
        ns_window.makeKeyAndOrderFront(None);
        NSAnimationContext::beginGrouping();
        let context = NSAnimationContext::currentContext();
        context.setDuration(SHOW_DURATION_SECS);
        let animator = ns_window.animator();
        animator.setFrame_display(frame, true);
        NSAnimationContext::endGrouping();
        let actual = ns_window.frame();
        crate::logger::log(&format!(
            "panel: AppKit mouse=({:.0},{:.0}) visible=({:.0},{:.0},{:.0},{:.0}) actual=({:.0},{:.0},{:.0},{:.0}) [points]",
            mouse.x, mouse.y,
            visible.origin.x, visible.origin.y, visible.size.width, visible.size.height,
            actual.origin.x, actual.origin.y, actual.size.width, actual.size.height,
        ));
        let _ = emit_window.emit("panel-shown", ());
    });
    if let Err(error) = result {
        crate::logger::log(&format!("panel: AppKit dispatch failed: {}", error));
    }
}

/// 非 macOS 平台继续使用 Tauri 原生物理坐标 API。
#[cfg(not(target_os = "macos"))]
pub fn show_panel(window: &tauri::WebviewWindow) {
    position_at_bottom(window);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("panel-shown", ());
}

#[cfg(target_os = "macos")]
pub fn hide_panel(window: &tauri::WebviewWindow) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAnimatablePropertyContainer, NSAnimationContext, NSWindow};
    use objc2_foundation::{NSPoint, NSRect};

    let transition_id = PANEL_TRANSITION_ID.fetch_add(1, Ordering::SeqCst) + 1;
    let target_window = window.clone();
    let emit_window = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Some(_mtm) = MainThreadMarker::new() else {
            return;
        };
        let Ok(raw_window) = target_window.ns_window() else {
            return;
        };
        let ns_window = unsafe { &*raw_window.cast::<NSWindow>() };
        let current = ns_window.frame();
        let offscreen = NSRect::new(
            NSPoint::new(current.origin.x, current.origin.y - current.size.height - 5.0),
            current.size,
        );
        NSAnimationContext::beginGrouping();
        let context = NSAnimationContext::currentContext();
        context.setDuration(HIDE_DURATION_SECS);
        let animator = ns_window.animator();
        animator.setFrame_display(offscreen, true);
        NSAnimationContext::endGrouping();
        let _ = emit_window.emit("panel-closing", ());
    });

    let hide_window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(130));
        if PANEL_TRANSITION_ID.load(Ordering::SeqCst) == transition_id {
            let final_window = hide_window.clone();
            let _ = hide_window.run_on_main_thread(move || {
                let _ = final_window.hide();
                let _ = final_window.emit("panel-hidden", ());
            });
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn hide_panel(window: &tauri::WebviewWindow) {
    let _ = window.hide();
    let _ = window.emit("panel-hidden", ());
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
                            hide_panel(&window);
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
                            show_panel(&window);
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
