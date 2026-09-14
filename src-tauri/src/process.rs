//! 进程快照与可执行文件选择。
//!
//! 契约（plan §6）：ProcessInfo { pid, name, path, title, hasWindow, icon? }。
//! Windows 用 sysinfo 枚举（不 spawn tasklist）；窗口标题用 Win32 EnumWindows 采集；
//! 权限受限进程保留名字，路径留空或部分。icon 暂不采集（验收边界：不宣称实现图标）。

use crate::error::AppResult;
use serde::{Deserialize, Serialize};

/// 前端契约的进程信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub title: String,
    pub has_window: bool,
}

/// 内部评估用的进程条目（rule engine 消费）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
    pub path: String,
    pub title: String,
    pub has_window: bool,
}

impl From<&ProcessEntry> for ProcessInfo {
    fn from(e: &ProcessEntry) -> Self {
        ProcessInfo {
            pid: e.pid,
            name: e.name.clone(),
            path: e.path.clone(),
            title: e.title.clone(),
            has_window: e.has_window,
        }
    }
}

/// 列出全部进程（Windows 真实枚举；测试在其他平台返回空/mock 注入）。
pub fn list_processes() -> AppResult<Vec<ProcessInfo>> {
    let entries = snapshot_entries();
    let mut list: Vec<ProcessInfo> = entries.iter().map(Into::into).collect();
    list.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    list.dedup_by(|a, b| a.pid == b.pid);
    Ok(list)
}

/// 一次进程快照（供监控循环与 list_processes 共用实现）。
pub fn snapshot_entries() -> Vec<ProcessEntry> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    #[cfg(target_os = "windows")]
    let titles = window_titles_by_pid();
    #[cfg(not(target_os = "windows"))]
    let titles: std::collections::HashMap<u32, String> = Default::default();

    let mut out = Vec::new();
    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();
        let name = process.name().to_string_lossy().into_owned();
        if name.is_empty() {
            continue;
        }
        let path = process
            .exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let title = titles.get(&pid_u32).cloned().unwrap_or_default();
        let has_window = !title.is_empty();
        out.push(ProcessEntry {
            pid: pid_u32,
            name,
            path,
            title,
            has_window,
        });
    }
    out
}

#[cfg(target_os = "windows")]
mod win {
    use std::collections::HashMap;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    /// 枚举可见顶层窗口，按 PID 汇总第一个窗口标题。
    pub(super) fn window_titles_by_pid() -> HashMap<u32, String> {
        let mut map: HashMap<u32, String> = HashMap::new();
        let ptr = &mut map as *mut HashMap<u32, String>;
        unsafe {
            let _ = EnumWindows(
                Some(enum_proc),
                LPARAM(ptr as isize),
            );
        }
        map
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let map = &mut *(lparam.0 as *mut HashMap<u32, String>);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return BOOL(1);
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied > 0 {
            let title = String::from_utf16_lossy(&buf[..copied as usize]);
            map.entry(pid).or_insert(title);
        }
        BOOL(1)
    }
}

#[cfg(target_os = "windows")]
use win::window_titles_by_pid;

// ---------- 文件选择（原生对话框，非 HTML input） ----------
//
// async command 运行在线程池（非主线程），官方文档推荐在异步上下文使用 blocking_* API。

use tauri_plugin_dialog::DialogExt;

/// 打开文件选择对话框（exe/json/ini）。取消返回 None。
pub fn pick_file_blocking(app: &tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("可执行与配置文件", &["exe", "json", "ini"])
        .add_filter("可执行文件", &["exe"])
        .add_filter("JSON 配置", &["json"])
        .add_filter("INI 配置", &["ini"])
        .add_filter("所有文件", &["*"])
        .blocking_pick_file()
        .map(|f| f.to_string())
}

/// 保存对话框（导出配置）。
pub fn save_file_blocking(app: &tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("JSON 配置", &["json"])
        .set_file_name("soundpilot-export.json")
        .blocking_save_file()
        .map(|f| f.to_string())
}

/// pick_executable 命令：选择一个 EXE，返回 ProcessInfo 形状（pid=0）。
pub fn pick_executable_blocking(app: &tauri::AppHandle) -> Option<ProcessInfo> {
    let path = app
        .dialog()
        .file()
        .add_filter("可执行文件", &["exe"])
        .blocking_pick_file()
        .map(|f| f.to_string())?;
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&path)
        .to_string();
    Some(ProcessInfo {
        pid: 0,
        name,
        path,
        title: String::new(),
        has_window: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_info_serializes_camel_case() {
        let pi = ProcessInfo {
            pid: 12,
            name: "a.exe".into(),
            path: "C:\\a.exe".into(),
            title: "T".into(),
            has_window: true,
        };
        let s = serde_json::to_string(&pi).unwrap();
        assert!(s.contains("\"hasWindow\":true"));
        assert!(s.contains("\"pid\":12"));
    }

    #[test]
    fn entry_to_info_maps() {
        let e = ProcessEntry {
            pid: 7,
            name: "n".into(),
            path: "p".into(),
            title: "t".into(),
            has_window: false,
        };
        let i: ProcessInfo = (&e).into();
        assert_eq!(i.pid, 7);
        assert!(!i.has_window);
    }
}
