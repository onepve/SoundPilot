//! 配置模型：JSON camelCase、校验、原子保存、损坏报错不覆盖、INI 导入、脱敏导出。
//!
//! 契约（soundpilot-implementation-plan.md §3-4）：
//! Config { version:1, serverUrl, token, speakers, normalVolume, matchMode:'order'|'volume',
//! restoreMode:'normal'|'previous'|'none', checkInterval, exitDebounce, startPaused, autoStart, rules }
//! Rule { id, name, kind:'game'|'platform', enabled, volume, processes:[{name,path}] }

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "soundpilot.json";
pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub server_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    pub speakers: Vec<Speaker>,
    pub normal_volume: u8,
    pub match_mode: MatchMode,
    pub restore_mode: RestoreMode,
    /// 秒
    pub check_interval: u64,
    /// 秒（进程退出后恢复延迟）
    pub exit_debounce: u64,
    pub start_paused: bool,
    pub auto_start: bool,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Speaker {
    pub did: String,
    pub name: String,
    pub enabled: bool,
}

// 与前端 types.ts 契约一致：小写字符串（order / volume）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    /// 同组按数组顺序取第一个命中
    #[default]
    Order,
    /// 同组取最高音量的规则
    Volume,
}

impl MatchMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchMode::Order => "order",
            MatchMode::Volume => "volume",
        }
    }
}

// 与前端 types.ts 契约一致：小写字符串（normal / previous / none）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RestoreMode {
    /// 恢复到 normalVolume
    #[default]
    Normal,
    /// 恢复到启动时备份的原音量
    Previous,
    /// 不恢复
    None,
}

impl RestoreMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            RestoreMode::Normal => "normal",
            RestoreMode::Previous => "previous",
            RestoreMode::None => "none",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub kind: RuleKind,
    pub enabled: bool,
    /// 0..=100
    pub volume: u8,
    pub processes: Vec<ProcessPattern>,
}

// 与前端 types.ts 契约一致：小写字符串（game / platform）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    Game,
    Platform,
}

impl RuleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleKind::Game => "game",
            RuleKind::Platform => "platform",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessPattern {
    /// 进程名（不含路径），匹配时不区分大小写
    pub name: String,
    /// 非空时精准路径匹配（不区分大小写、统一分隔符）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
}

impl Config {
    /// 首次无配置 / 配置损坏时的默认值：默认启用监控（startPaused=false）。
    /// 未配置音箱与服务器时不会发送任何音量请求。
    pub fn safe_default() -> Self {
        Config {
            version: CONFIG_VERSION,
            server_url: String::new(),
            token: String::new(),
            speakers: Vec::new(),
            normal_volume: 30,
            match_mode: MatchMode::Order,
            restore_mode: RestoreMode::Normal,
            check_interval: 5,
            exit_debounce: 10,
            start_paused: false,
            auto_start: false,
            rules: Vec::new(),
        }
    }

    /// 校验配置；返回人类可读错误（不含 token）。
    pub fn validate(&self) -> AppResult<()> {
        if self.version != CONFIG_VERSION {
            return Err(AppError::Config(format!("不支持的配置版本 {}", self.version)));
        }
        if self.normal_volume > 100 {
            return Err(AppError::Config("normalVolume 必须在 0..100".into()));
        }
        if self.check_interval == 0 {
            return Err(AppError::Config("checkInterval 必须大于 0".into()));
        }
        if self.exit_debounce > 3600 {
            return Err(AppError::Config("exitDebounce 过大（最大 3600 秒）".into()));
        }
        let mut dids = std::collections::HashSet::new();
        for s in &self.speakers {
            if s.did.trim().is_empty() {
                return Err(AppError::Config("音箱 did 不能为空".into()));
            }
            if !dids.insert(s.did.clone()) {
                return Err(AppError::Config(format!("音箱 did 重复: {}", s.did)));
            }
        }
        let mut ids = std::collections::HashSet::new();
        for r in &self.rules {
            if r.id.trim().is_empty() {
                return Err(AppError::Config("规则 id 不能为空".into()));
            }
            if !ids.insert(r.id.clone()) {
                return Err(AppError::Config(format!("规则 id 重复: {}", r.id)));
            }
            if r.volume > 100 {
                return Err(AppError::Config(format!("规则 {} 音量需在 0..100", r.name)));
            }
            if r.processes.is_empty() {
                return Err(AppError::Config(format!("规则 {} 至少需要 1 个进程", r.name)));
            }
            for p in &r.processes {
                if p.name.trim().is_empty() {
                    return Err(AppError::Config(format!(
                        "规则 {} 存在空进程名",
                        r.name
                    )));
                }
            }
        }
        Ok(())
    }
}

/// 配置文件路径：EXE 同目录优先（存在配置或便携标记），回落 %APPDATA%\SoundPilot。
pub fn config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(CONFIG_FILE);
            if p.exists() || dir.join("soundpilot.portable").exists() {
                return p;
            }
        }
    }
    app_config_dir().join(CONFIG_FILE)
}

fn app_config_dir() -> PathBuf {
    // 不引入 dirs crate：Windows %APPDATA%\SoundPilot，其余 XDG 兼容
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("SoundPilot");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("soundpilot");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config").join("soundpilot");
        }
    }
    PathBuf::from(".").join("soundpilot-config")
}

/// 读取配置：文件不存在 → Err(ConfigNotFound)（调用方决定写默认）；
/// 文件存在但损坏 → Err（明确报错，不覆盖）。
pub fn load_config() -> AppResult<Config> {
    let path = config_path();
    load_config_from(&path)
}

pub fn load_config_from(path: &Path) -> AppResult<Config> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // 首次：生成安全默认（原子写）
            let def = Config::safe_default();
            save_config_to(path, &def)?;
            return Ok(def);
        }
        Err(e) => return Err(AppError::Config(format!("读取配置失败: {e}"))),
    };
    parse_config(&raw).map_err(|e| {
        AppError::Config(format!(
            "配置文件损坏（未覆盖，请修复或删除后重启）: {path:?}: {e}"
        ))
    })
}

/// 解析 + 校验。损坏的 JSON 或非法字段直接报错。
pub fn parse_config(raw: &str) -> AppResult<Config> {
    let cfg: Config = serde_json::from_str(raw)
        .map_err(|e| AppError::Config(format!("JSON 解析失败: {e}")))?;
    cfg.validate()?;
    Ok(cfg)
}

/// 原子保存：写临时文件 + rename；并保留 .bak。
pub fn save_config_to(path: &Path, cfg: &Config) -> AppResult<()> {
    cfg.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| AppError::Config(format!("序列化失败: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    if path.exists() {
        let bak = path.with_extension("json.bak");
        let _ = fs::copy(path, &bak);
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

// ---------- INI 导入 ----------

/// 导入结果：解析出的 Config（供前端预览确认，不落盘）。
pub type ImportedConfig = Config;

/// 从文件导入 INI 或 JSON（按扩展名/内容嗅探）。
pub fn import_config_file(path: &Path) -> AppResult<ImportedConfig> {
    let raw = fs::read_to_string(path)
        .map_err(|e| AppError::Io(format!("读取导入文件失败: {e}")))?;
    let trimmed = raw.trim_start();
    if trimmed.starts_with('{') {
        return parse_config(&raw);
    }
    crate::ini::parse_ini_config(&raw)
}

/// 导出：Token 清空后写出；返回导出路径字符串。
pub fn export_config_to(path: &Path, cfg: &Config) -> AppResult<String> {
    let mut sanitized = cfg.clone();
    sanitized.token = String::new();
    save_config_to(path, &sanitized)?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Config {
        Config {
            version: 1,
            server_url: "https://example.com".into(),
            token: "SECRET".into(),
            speakers: vec![Speaker {
                did: "12345".into(),
                name: "客厅".into(),
                enabled: true,
            }],
            normal_volume: 30,
            match_mode: MatchMode::Order,
            restore_mode: RestoreMode::Normal,
            check_interval: 5,
            exit_debounce: 10,
            start_paused: false,
            auto_start: false,
            rules: vec![Rule {
                id: "r1".into(),
                name: "游戏".into(),
                kind: RuleKind::Game,
                enabled: true,
                volume: 15,
                processes: vec![ProcessPattern {
                    name: "game.exe".into(),
                    path: String::new(),
                }],
            }],
        }
    }

    #[test]
    fn json_is_camel_case() {
        let s = serde_json::to_string(&sample()).unwrap();
        assert!(s.contains("\"serverUrl\""));
        assert!(s.contains("\"normalVolume\""));
        assert!(s.contains("\"matchMode\":\"order\""));
        assert!(s.contains("\"restoreMode\":\"normal\""));
        assert!(s.contains("\"startPaused\""));
        assert!(s.contains("\"checkInterval\""));
        assert!(s.contains("\"exitDebounce\""));
        // Rule 字段
        assert!(s.contains("\"kind\":\"game\""));
        assert!(s.contains("\"processes\":[{\"name\":\"game.exe\"}]"));
    }

    #[test]
    fn parse_roundtrip() {
        let s = serde_json::to_string(&sample()).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(back, sample());
    }

    #[test]
    fn validate_rejects_bad() {
        let mut c = sample();
        c.normal_volume = 101;
        assert!(c.validate().is_err());
        let mut c = sample();
        c.check_interval = 0;
        assert!(c.validate().is_err());
        let mut c = sample();
        c.version = 2;
        assert!(c.validate().is_err());
        let mut c = sample();
        c.rules[0].volume = 200;
        assert!(c.validate().is_err());
        let mut c = sample();
        c.speakers.push(Speaker {
            did: "12345".into(),
            name: "dup".into(),
            enabled: true,
        });
        assert!(c.validate().is_err());
    }

    #[test]
    fn load_missing_creates_safe_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        let cfg = load_config_from(&path).unwrap();
        assert!(!cfg.start_paused, "测试包首次默认必须 startPaused=false（默认启用监控）");
        assert!(path.exists(), "默认配置应已生成");
        // 再读一致
        let cfg2 = load_config_from(&path).unwrap();
        assert_eq!(cfg, cfg2);
    }

    #[test]
    fn corrupt_config_errors_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        fs::write(&path, "{ not json !!!").unwrap();
        let err = load_config_from(&path).unwrap_err();
        assert!(err.to_string().contains("损坏"), "应报损坏错误: {err}");
        // 原损坏内容仍在，未被覆盖
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json !!!");
    }

    #[test]
    fn save_is_atomic_and_baks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        let c1 = sample();
        save_config_to(&path, &c1).unwrap();
        let mut c2 = sample();
        c2.normal_volume = 42;
        save_config_to(&path, &c2).unwrap();
        assert!(path.with_extension("json.bak").exists(), "应生成 .bak");
        let loaded = load_config_from(&path).unwrap();
        assert_eq!(loaded.normal_volume, 42);
        // 不残留 tmp
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn export_strips_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        let out = export_config_to(&path, &sample()).unwrap();
        assert!(out.contains("export.json"));
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("SECRET"), "导出不得含 token");
        let cfg: Config = serde_json::from_str(&raw).unwrap();
        assert_eq!(cfg.token, "");
    }
}
