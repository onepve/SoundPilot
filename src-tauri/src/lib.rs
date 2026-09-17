//! SoundPilot 后端库：配置、规则引擎、进程快照、HTTP 客户端、串行监控 actor 与 Tauri 集成。

pub mod config;
pub mod error;
pub mod http_client;
pub mod ini;
pub mod monitor;
pub mod process;
pub mod rule;
pub mod state;

use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

use crate::config::{load_config, Config};
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // 单实例插件必须是第一个注册的插件（官方要求）
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 第二个实例：唤起已有主窗口
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        // 开机自启：注册时附加 MINIMIZED_FLAG，启动后据此只驻留托盘
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_FLAG]),
        ))
        .setup(|app| {
            let config = load_config().unwrap_or_else(|err| {
                // 损坏配置：不覆盖，使用安全默认（startPaused=true）并记录到事件日志
                eprintln!("[soundpilot] config load failed, using safe default: {err}");
                Config::safe_default()
            });
            let state = Arc::new(AppState::new(config));
            if state.config().read().unwrap().start_paused {
                state.pause_monitor(None);
                state.log_event("info", "startPaused=true：监控已安全暂停（不会写音箱）");
            }
            app.manage(state.clone());

            // 监控 actor（串行队列：检测→写→核验，全部经单一任务执行）
            let mon = monitor::MonitorHandle::new(state.clone());
            app.manage(mon.clone());

            // 启动备份原音量（restoreMode=previous 用；只读不写）
            let backup_state = state.clone();
            tauri::async_runtime::spawn(async move {
                backup_state.backup_baseline_volumes().await;
            });

            build_tray(app)?;

            // 开机自启：把配置状态同步到系统（注册表 / LaunchAgent），失败仅记录不阻断
            let auto_start = state.config().read().unwrap().auto_start;
            if let Err(e) = sync_autostart(app.handle(), auto_start) {
                state.log_event("warn", &format!("开机自启设置同步失败：{e}"));
            }

            // 由开机自启拉起（带 --minimized）时只驻留托盘，不弹窗打扰
            let start_minimized = is_minimized_launch(&std::env::args().collect::<Vec<String>>());
            if let Some(win) = app.get_webview_window("main") {
                if start_minimized {
                    let _ = win.hide();
                    state.log_event("info", "开机自启启动：已最小化到托盘");
                } else {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关闭窗口 → 隐藏到托盘（真正退出走托盘菜单/quit_app）
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            cmd::get_config,
            cmd::save_config,
            cmd::get_status,
            cmd::list_processes,
            cmd::pause_monitor,
            cmd::resume_monitor,
            cmd::refresh_volumes,
            cmd::set_manual_volume,
            cmd::pick_executable,
            cmd::import_config,
            cmd::export_config,
            cmd::clear_logs,
            cmd::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // 事件循环结束前（RunEvent::Exit 在 tao process::exit 之前触发）统一收尾：
    // 托盘「退出」、quit_app、系统关机（WM_ENDSESSION）三路都汇聚到这一处，
    // 在这里等 actor 完成音量恢复，覆盖全部退出路径。
    let exit_code = app.run_return(|app, event| {
        if let tauri::RunEvent::Exit = event {
            restore_volumes_before_exit(app);
        }
    });
    std::process::exit(exit_code);
}

/// 退出收尾：标记 shutdown（拦在途写）→ actor 入队恢复（FIFO 尾部执行）→ 等完成。
/// 幂等：重复调用安全（Restore 入队失败即返回）。
fn restore_volumes_before_exit(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        if !state.is_shutting_down() {
            state.begin_shutdown();
        }
    }
    if let Some(m) = app.try_state::<monitor::MonitorHandle>() {
        m.restore_and_wait(std::time::Duration::from_secs(3));
    }
}

/// 开机自启注册时附加的启动参数：带此参数启动表示「静默驻留托盘」。
pub const MINIMIZED_FLAG: &str = "--minimized";

/// 启动参数是否要求最小化到托盘（开机自启路径）。
pub fn is_minimized_launch(args: &[String]) -> bool {
    args.iter().any(|a| a == MINIMIZED_FLAG)
}

/// 把「开机自启」开关同步到系统（Windows 注册表 Run / macOS LaunchAgent）。
/// 已是目标状态则不重复写入；失败返回错误文本，由调用方记录。
pub fn sync_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let current = manager.is_enabled().unwrap_or(false);
    if enabled == current {
        return Ok(());
    }
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

/// 托盘：打开/暂停/恢复/退出；左键单击打开主窗口。
/// （托盘仅在 Rust 构建，不经 tauri.conf.json，避免重复托盘图标）
fn build_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开主窗口", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "暂停监控", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "恢复监控", true, None::<&str>)?;
    let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pause, &resume, &sep, &quit])?;

    let state_for_menu: Arc<AppState> = app.state::<Arc<AppState>>().inner().clone();

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("小爱音量助手")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "pause" => state_for_menu.pause_monitor(None),
            "resume" => state_for_menu.resume_monitor(),
            "quit" => {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    state.begin_shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 左键单击（抬起）打开主窗口
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// 退出前清理：仅标记 shutdown（使在途写失效）。
/// 音量恢复由 RunEvent::Exit 统一执行（quit_app → app.exit → Exit 事件）。
fn request_quit(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        state.begin_shutdown();
    }
}

/// Tauri 命令层（thin wrapper；逻辑在库层供测试）
mod cmd {
    use super::*;
    use crate::error::AppError;

    #[tauri::command]
    pub fn get_config(state: tauri::State<'_, Arc<AppState>>) -> Result<Config, AppError> {
        Ok(state.config().read().unwrap().clone())
    }

    #[tauri::command]
    pub fn save_config(
        app: tauri::AppHandle,
        monitor: tauri::State<'_, monitor::MonitorHandle>,
        state: tauri::State<'_, Arc<AppState>>,
        config: Config,
    ) -> Result<(), AppError> {
        let auto_start = config.auto_start;
        state.save_config(config)?;
        // 开机自启开关保存后立即生效（含"--minimized"参数，登录后静默驻留托盘）
        if let Err(e) = sync_autostart(&app, auto_start) {
            state.log_event("warn", &format!("开机自启设置同步失败：{e}"));
        }
        monitor.check_now(); // 配置变更后立即重评估
        Ok(())
    }

    #[tauri::command]
    pub fn get_status(state: tauri::State<'_, Arc<AppState>>) -> Result<state::Status, AppError> {
        Ok(state.status())
    }

    #[tauri::command]
    pub fn list_processes() -> Result<Vec<process::ProcessInfo>, AppError> {
        process::list_processes()
    }

    #[tauri::command]
    pub fn pause_monitor(
        state: tauri::State<'_, Arc<AppState>>,
        minutes: Option<u64>,
    ) -> Result<(), AppError> {
        state.pause_monitor(minutes);
        Ok(())
    }

    #[tauri::command]
    pub fn resume_monitor(state: tauri::State<'_, Arc<AppState>>) -> Result<(), AppError> {
        state.resume_monitor();
        Ok(())
    }

    #[tauri::command]
    pub async fn refresh_volumes(
        state: tauri::State<'_, Arc<AppState>>,
    ) -> Result<state::Status, AppError> {
        let st = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || st)
            .await
            .map_err(|e| AppError::Monitor(format!("{e}")))?
            .refresh_volumes()
            .await
            .map_err(AppError::Monitor)
    }

    /// 手动音量：先暂停（防自动抢音量），再经串行 actor 写入（防与自动写交错）。
    #[tauri::command]
    pub async fn set_manual_volume(
        monitor: tauri::State<'_, monitor::MonitorHandle>,
        state: tauri::State<'_, Arc<AppState>>,
        volume: u8,
        did: Option<String>,
    ) -> Result<state::Status, AppError> {
        state.pause_monitor(None); // bump generation：在途自动写全部失效
        let gen = state.current_generation();
        monitor
            .manual_volume(gen, volume, did)
            .await
            .map_err(AppError::Monitor)
    }

    #[tauri::command]
    pub async fn pick_executable(
        app: tauri::AppHandle,
        state: tauri::State<'_, Arc<AppState>>,
    ) -> Result<Option<process::ProcessInfo>, AppError> {
        // blocking_* 在 async command（线程池）中是官方推荐用法
        let picked = tauri::async_runtime::spawn_blocking({
            let app = app.clone();
            move || process::pick_executable_blocking(&app)
        })
        .await
        .map_err(|e| AppError::Io(format!("文件选择任务失败: {e}")))?;
        if picked.is_some() {
            state.log_event("info", "已选择可执行文件");
        }
        Ok(picked)
    }

    #[tauri::command]
    pub async fn import_config(
        app: tauri::AppHandle,
        state: tauri::State<'_, Arc<AppState>>,
    ) -> Result<Option<config::ImportedConfig>, AppError> {
        // 原生文件选择器；解析 INI 或 JSON；仅预览不保存（前端确认后 save_config）
        let path = tauri::async_runtime::spawn_blocking({
            let app = app.clone();
            move || process::pick_file_blocking(&app)
        })
        .await
        .map_err(|e| AppError::Io(format!("文件选择任务失败: {e}")))?;
        let Some(path) = path else {
            return Ok(None);
        };
        let imported = config::import_config_file(std::path::Path::new(&path))?;
        state.log_event("info", "配置已导入（预览，未保存）");
        Ok(Some(imported))
    }

    #[tauri::command]
    pub async fn export_config(
        app: tauri::AppHandle,
        state: tauri::State<'_, Arc<AppState>>,
    ) -> Result<Option<String>, AppError> {
        // 原生保存对话框；Token 清空后写出
        let path = tauri::async_runtime::spawn_blocking({
            let app = app.clone();
            move || process::save_file_blocking(&app)
        })
        .await
        .map_err(|e| AppError::Io(format!("保存对话框任务失败: {e}")))?;
        let Some(path) = path else {
            return Ok(None);
        };
        let cfg = state.config().read().unwrap().clone();
        let exported = config::export_config_to(std::path::Path::new(&path), &cfg)?;
        state.log_event("info", "配置已导出（Token 已清空）");
        Ok(Some(exported))
    }

    #[tauri::command]
    pub fn clear_logs(state: tauri::State<'_, Arc<AppState>>) -> Result<usize, AppError> {
        let cleared = state.clear_events();
        state.log_event("info", "日志已清空");
        Ok(cleared)
    }

    #[tauri::command]
    pub fn quit_app(app: tauri::AppHandle) -> Result<(), AppError> {
        request_quit(&app);
        app.exit(0);
        Ok(())
    }
}

#[cfg(test)]
mod launch_mode_tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn minimized_flag_detected() {
        // 开机自启注册时附加的参数
        assert!(is_minimized_launch(&args(&["soundpilot.exe", "--minimized"])));
        assert!(is_minimized_launch(&args(&["--minimized"])));
    }

    #[test]
    fn normal_launch_is_not_minimized() {
        assert!(!is_minimized_launch(&args(&["soundpilot.exe"])));
        assert!(!is_minimized_launch(&[]));
        // 相似但不相同的参数不得误判
        assert!(!is_minimized_launch(&args(&["--minimized-x"])));
        assert!(!is_minimized_launch(&args(&["--no-minimized"])));
    }
}
