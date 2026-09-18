//! 应用共享状态：配置、监控状态、事件日志、原音量备份。
//!
//! 契约（plan §5）：Status 不含 token；events 环形缓冲；
//! 暂停（可定时）/恢复；set_manual_volume 先暂停再写；启动备份原音量。

use crate::config::Config;
use crate::error::AppResult;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_EVENTS: usize = 200;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub did: String,
    pub name: String,
    pub volume: Option<u8>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    /// ISO-8601 本地时间
    pub time: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub paused: bool,
    /// 定时暂停恢复时间戳（ms）；null 表示非定时
    pub pause_until: Option<u64>,
    pub active_rule_id: Option<String>,
    pub active_rule_name: Option<String>,
    pub target_volume: Option<u8>,
    /// connected / disconnected / unknown
    pub connection: String,
    pub devices: Vec<DeviceStatus>,
    pub events: Vec<StatusEvent>,
    pub config_path: String,
    pub version: String,
}

/// 写代数：每次暂停/配置变更递增，用于让旧的异步写失效（防陈旧写）。
pub type Generation = u64;

pub struct AppState {
    config: RwLock<Config>,
    pub(crate) paused: AtomicBool,
    pub(crate) pause_until_ms: AtomicU64,
    /// 每音箱最近一次已知音量与错误
    pub(crate) devices: Mutex<HashMap<String, DeviceStatus>>,
    pub(crate) active_rule: Mutex<(Option<String>, Option<String>, Option<u8>)>,
    pub(crate) events: Mutex<VecDeque<StatusEvent>>,
    /// 启动时备份的原音量（restoreMode=previous 用）
    pub(crate) baseline_volumes: Mutex<HashMap<String, u8>>,
    /// 本次运行中已实际写入成功的音箱；无规则的手动下发退出恢复仅使用此集合。
    pub(crate) successfully_written_devices: Mutex<HashSet<String>>,
    /// 写代数（暂停/配置变更/手动写都会 bump）
    pub(crate) generation: AtomicU64,
    pub(crate) shutting_down: AtomicBool,
    pub(crate) connection: Mutex<String>,
    pub(crate) config_path: String,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let config_path = crate::config::config_path().to_string_lossy().into_owned();
        // startPaused 在状态层即生效（默认 false = 启动即启用监控）
        let start_paused = config.start_paused;
        AppState {
            config: RwLock::new(config),
            paused: AtomicBool::new(start_paused),
            pause_until_ms: AtomicU64::new(0),
            devices: Mutex::new(HashMap::new()),
            active_rule: Mutex::new((None, None, None)),
            events: Mutex::new(VecDeque::new()),
            baseline_volumes: Mutex::new(HashMap::new()),
            successfully_written_devices: Mutex::new(HashSet::new()),
            generation: AtomicU64::new(1),
            shutting_down: AtomicBool::new(false),
            connection: Mutex::new("unknown".into()),
            config_path,
        }
    }

    pub fn config(&self) -> &RwLock<Config> {
        &self.config
    }

    pub fn current_generation(&self) -> Generation {
        self.generation.load(Ordering::SeqCst)
    }

    pub fn bump_generation(&self) -> Generation {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn is_paused(&self) -> bool {
        if self.paused.load(Ordering::SeqCst) {
            // 定时暂停到期自动恢复
            let until = self.pause_until_ms.load(Ordering::SeqCst);
            if until > 0 {
                let now = now_ms();
                if now >= until {
                    self.resume_monitor();
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// 暂停监控。minutes=Some(n) 定时暂停；None 无限期。
    /// 暂停不恢复音量，仅停止自动调整（plan §8）。
    pub fn pause_monitor(&self, minutes: Option<u64>) {
        self.bump_generation();
        self.paused.store(true, Ordering::SeqCst);
        let until = minutes
            .filter(|m| *m > 0)
            .map(|m| now_ms() + m * 60_000);
        self.pause_until_ms
            .store(until.unwrap_or(0), Ordering::SeqCst);
        let msg = match minutes {
            Some(m) => format!("监控已暂停 {m} 分钟"),
            None => "监控已暂停".to_string(),
        };
        self.log_event("info", &msg);
    }

    /// 恢复监控：清暂停标记，规则重新识别（activeRule 重置为 null，等下一轮快照）。
    pub fn resume_monitor(&self) {
        self.bump_generation();
        self.paused.store(false, Ordering::SeqCst);
        self.pause_until_ms.store(0, Ordering::SeqCst);
        *self.active_rule.lock().unwrap() = (None, None, None);
        self.log_event("info", "监控已恢复");
    }

    pub fn begin_shutdown(&self) {
        self.bump_generation();
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    /// 仅在实际 HTTP 写入成功后记录；失败/排队/点击均不得触发退出恢复资格。
    pub fn record_successful_volume_write(&self, did: &str) {
        self.successfully_written_devices
            .lock()
            .unwrap()
            .insert(did.to_string());
    }

    pub fn successfully_written_devices(&self) -> HashSet<String> {
        self.successfully_written_devices.lock().unwrap().clone()
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// 保存配置（校验 + 原子写 + 状态更新）。配置变更使旧的写代失效。
    pub fn save_config(&self, new_config: Config) -> AppResult<()> {
        new_config.validate()?;
        crate::config::save_config_to(&crate::config::config_path(), &new_config)?;
        self.bump_generation();
        *self.config.write().unwrap() = new_config;
        *self.active_rule.lock().unwrap() = (None, None, None);
        self.log_event("info", "配置已保存并生效");
        Ok(())
    }

    pub fn log_event(&self, level: &str, message: &str) {
        let mut ev = self.events.lock().unwrap();
        ev.push_back(StatusEvent {
            time: iso_now(),
            level: level.to_string(),
            message: message.to_string(),
        });
        while ev.len() > MAX_EVENTS {
            ev.pop_front();
        }
    }

    /// 清空日志缓冲（仅内存，不涉及磁盘）。返回清空的条数。
    pub fn clear_events(&self) -> usize {
        let mut ev = self.events.lock().unwrap();
        let n = ev.len();
        ev.clear();
        n
    }

    pub fn set_devices_status(&self, devices: Vec<DeviceStatus>) {
        let mut d = self.devices.lock().unwrap();
        d.clear();
        for dev in devices {
            d.insert(dev.did.clone(), dev);
        }
    }

    pub fn set_connection(&self, conn: &str) {
        *self.connection.lock().unwrap() = conn.to_string();
    }

    pub fn set_baseline(&self, did: &str, volume: u8) {
        self.baseline_volumes
            .lock()
            .unwrap()
            .insert(did.to_string(), volume);
    }

    pub fn baseline(&self, did: &str) -> Option<u8> {
        self.baseline_volumes.lock().unwrap().get(did).copied()
    }

    pub fn set_active_rule(&self, id: Option<String>, name: Option<String>, target: Option<u8>) {
        *self.active_rule.lock().unwrap() = (id, name, target);
    }

    pub fn status(&self) -> Status {
        let cfg = self.config.read().unwrap();
        let devices_map = self.devices.lock().unwrap();
        let devices: Vec<DeviceStatus> = cfg
            .speakers
            .iter()
            .map(|s| devices_map.get(&s.did).cloned().unwrap_or(DeviceStatus {
                did: s.did.clone(),
                name: s.name.clone(),
                volume: None,
                error: None,
            }))
            .collect();
        let (rule_id, rule_name, target) = self.active_rule.lock().unwrap().clone();
        let paused_now = self.is_paused();
        let until = self.pause_until_ms.load(Ordering::SeqCst);
        Status {
            paused: paused_now,
            pause_until: if paused_now && until > 0 { Some(until) } else { None },
            active_rule_id: rule_id,
            active_rule_name: rule_name,
            target_volume: target,
            connection: self.connection.lock().unwrap().clone(),
            devices,
            events: self.events.lock().unwrap().iter().cloned().collect(),
            config_path: self.config_path.clone(),
            version: APP_VERSION.to_string(),
        }
    }

    /// 手动音量命令路径：先暂停（bump generation），再取最新代数经 actor 串行写入。
    /// 实现在 monitor.rs（manual_volume_impl），此处提供状态更新辅助。
    pub fn update_devices(&self, devices: Vec<DeviceStatus>) {
        let mut d = self.devices.lock().unwrap();
        for dev in devices {
            d.insert(dev.did.clone(), dev);
        }
    }

    /// 仅内存生效的配置替换（测试与导入预览用；不写盘）。
    pub fn apply_config_in_memory(&self, cfg: Config) {
        self.bump_generation();
        *self.config.write().unwrap() = cfg;
        *self.active_rule.lock().unwrap() = (None, None, None);
    }

    /// 读取全部启用音箱音量（refresh_volumes 命令与启动备份共用）。
    pub async fn refresh_volumes(self: &Arc<Self>) -> Result<Status, String> {
        let cfg = self.config.read().unwrap().clone();
        let enabled: Vec<_> = cfg.speakers.iter().filter(|s| s.enabled).collect();
        if enabled.is_empty() {
            self.set_connection("disconnected");
            return Ok(self.status());
        }
        let mut devices = Vec::new();
        let mut all_ok = true;
        for s in enabled {
            let client = crate::http_client::SpeakerClient::new(&cfg.server_url, &cfg.token);
            let r = client.get_volume(&s.did).await;
            let dev = match r {
                Ok(v) => DeviceStatus {
                    did: s.did.clone(),
                    name: s.name.clone(),
                    volume: Some(v),
                    error: None,
                },
                Err(e) => {
                    all_ok = false;
                    DeviceStatus {
                        did: s.did.clone(),
                        name: s.name.clone(),
                        volume: None,
                        error: Some(e.to_string()),
                    }
                }
            };
            devices.push(dev);
        }
        self.set_connection(if all_ok { "connected" } else { "disconnected" });
        self.set_devices_status(devices);
        if all_ok {
            self.log_event("info", "已刷新音箱音量");
        }
        Ok(self.status())
    }

    /// 启动时备份原音量（restoreMode=previous 用；失败不阻塞启动）。
    pub async fn backup_baseline_volumes(&self) {
        let cfg = self.config.read().unwrap().clone();
        for s in cfg.speakers.iter().filter(|s| s.enabled) {
            let client = crate::http_client::SpeakerClient::new(&cfg.server_url, &cfg.token);
            if let Ok(v) = client.get_volume(&s.did).await {
                self.set_baseline(&s.did, v);
            }
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn iso_now() -> String {
    // 无 chrono：用 UNIX 秒生成 UTC ISO 时间（够用；前端可格式化）
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant 的 days→civil 算法。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, MatchMode, RestoreMode, Speaker};

    fn test_config() -> Config {
        Config {
            version: 1,
            server_url: String::new(),
            token: String::new(),
            speakers: vec![Speaker {
                did: "d1".into(),
                name: "测试".into(),
                enabled: true,
            }],
            normal_volume: 30,
            match_mode: MatchMode::Order,
            restore_mode: RestoreMode::Normal,
            check_interval: 5,
            exit_debounce: 10,
            start_paused: false,
            auto_start: false,
            rules: vec![],
        }
    }

    #[test]
    fn status_shape_and_camel_case() {
        let st = AppState::new(test_config());
        let s = serde_json::to_value(st.status()).unwrap();
        for key in [
            "paused",
            "pauseUntil",
            "activeRuleId",
            "activeRuleName",
            "targetVolume",
            "connection",
            "devices",
            "events",
            "configPath",
            "version",
        ] {
            assert!(s.get(key).is_some(), "Status 缺少字段 {key}");
        }
        assert_eq!(s["version"], serde_json::json!(APP_VERSION));
    }

    #[test]
    fn pause_resume_and_generation() {
        let st = AppState::new(test_config());
        assert!(!st.is_paused());
        let g0 = st.current_generation();
        st.pause_monitor(None);
        assert!(st.is_paused());
        assert!(st.current_generation() > g0, "暂停必须 bump generation");
        st.resume_monitor();
        assert!(!st.is_paused());
    }

    #[test]
    fn timed_pause_expires() {
        let st = AppState::new(test_config());
        // 用已过去的时间模拟定时到期：直接写 pause_until
        st.pause_monitor(Some(10));
        assert!(st.is_paused());
        st.pause_until_ms.store(1, Ordering::SeqCst); // epoch 后 1ms 必然已过期
        assert!(!st.is_paused(), "到期后应自动恢复");
    }

    #[test]
    fn events_are_capped() {
        let st = AppState::new(test_config());
        for i in 0..(MAX_EVENTS + 50) {
            st.log_event("info", &format!("e{i}"));
        }
        assert_eq!(st.events.lock().unwrap().len(), MAX_EVENTS);
    }

    #[test]
    fn clear_events_empties_buffer() {
        let st = AppState::new(test_config());
        for i in 0..5 {
            st.log_event("info", &format!("e{i}"));
        }
        assert_eq!(st.clear_events(), 5, "应返回被清空的条数");
        assert!(st.events.lock().unwrap().is_empty());
        // 清空后可继续记录
        st.log_event("info", "清除后又写入");
        assert_eq!(st.events.lock().unwrap().len(), 1);
        assert_eq!(st.clear_events(), 1);
        assert_eq!(st.clear_events(), 0, "空缓冲清空返回 0");
    }

    #[test]
    fn iso_now_format() {
        let t = iso_now();
        assert!(t.len() == 20 && t.ends_with('Z'), "{t}");
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
    }

    #[test]
    fn civil_days_known_date() {
        // 2026-09-14 是自 epoch 起 20710 天
        assert_eq!(civil_from_days(20710), (2026, 9, 14));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn status_never_contains_token() {
        let mut c = test_config();
        c.token = "SUPERSECRET-TOKEN-XYZ".into();
        c.server_url = "https://x.example".into();
        let st = AppState::new(c);
        let s = serde_json::to_string(&st.status()).unwrap();
        assert!(!s.contains("SUPERSECRET"), "Status 不得包含 token");
    }

    #[test]
    fn active_rule_set_get() {
        let st = AppState::new(test_config());
        st.set_active_rule(Some("r1".into()), Some("游戏".into()), Some(15));
        let s = st.status();
        assert_eq!(s.active_rule_id.as_deref(), Some("r1"));
        assert_eq!(s.target_volume, Some(15));
    }

    #[test]
    fn device_status_camel_case() {
        let d = DeviceStatus {
            did: "x".into(),
            name: "n".into(),
            volume: Some(3),
            error: None,
        };
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"volume\":3"));
    }

    #[test]
    fn baseline_roundtrip() {
        let st = AppState::new(test_config());
        st.set_baseline("d1", 42);
        assert_eq!(st.baseline("d1"), Some(42));
        assert_eq!(st.baseline("other"), None);
    }
}
