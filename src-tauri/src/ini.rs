//! 旧版 config.ini 导入解析（仅预览，不保存）。
//!
//! 兼容 xiaoi 旧版结构示例：
//! [general] server_url= / token= / normal_volume= / check_interval= / exit_debounce=
//! [speaker.<did>] name= enabled=
//! [rule.<id>] name= kind=game|platform enabled= volume= processes=game.exe;other.exe|C:\path\a.exe

use crate::config::{
    Config, MatchMode, ProcessPattern, RestoreMode, Rule, RuleKind, Speaker,
};
use crate::error::AppResult;
use std::collections::BTreeMap;

/// 极简 INI 解析：section 头 `[name]`，`key = value`，`;`/`#` 注释。
pub fn parse_ini(raw: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut section = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('[') {
            if let Some(end) = stripped.strip_suffix(']') {
                section = end.trim().to_ascii_lowercase();
                out.entry(section.clone()).or_default();
            }
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.entry(section.clone())
                .or_default()
                .insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    out
}

fn get_u64(m: &BTreeMap<String, String>, key: &str, default: u64) -> u64 {
    m.get(key)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn get_bool(m: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    match m.get(key).map(|s| s.to_ascii_lowercase()) {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes" | "on"),
        None => default,
    }
}

/// 把 INI 文本解析为 Config（缺省字段取安全默认；startPaused 强制 true 以保证导入后安全）。
pub fn parse_ini_config(raw: &str) -> AppResult<Config> {
    let ini = parse_ini(raw);
    let general = ini.get("general").cloned().unwrap_or_default();

    let server_url = general
        .get("server_url")
        .or_else(|| general.get("serverurl"))
        .or_else(|| general.get("url"))
        .cloned()
        .unwrap_or_default();
    let token = general
        .get("token")
        .or_else(|| general.get("auth_token"))
        .cloned()
        .unwrap_or_default();

    let normal_volume = get_u64(&general, "normal_volume", 30).min(100) as u8;
    let check_interval = get_u64(&general, "check_interval", 5).max(1);
    let exit_debounce = get_u64(&general, "exit_debounce", 10).min(3600);

    let match_mode = match general
        .get("match_mode")
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("volume") => MatchMode::Volume,
        _ => MatchMode::Order,
    };
    let restore_mode = match general
        .get("restore_mode")
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("previous") => RestoreMode::Previous,
        Some("none") => RestoreMode::None,
        _ => RestoreMode::Normal,
    };

    // speakers: [speaker.<did>]
    let mut speakers = Vec::new();
    for (sec, kv) in &ini {
        if let Some(did) = sec.strip_prefix("speaker.") {
            if did.trim().is_empty() {
                continue;
            }
            speakers.push(Speaker {
                did: did.to_string(),
                name: kv.get("name").cloned().unwrap_or_else(|| did.to_string()),
                enabled: get_bool(kv, "enabled", true),
            });
        }
    }

    // rules: [rule.<id>]
    let mut rules = Vec::new();
    for (sec, kv) in &ini {
        if let Some(id) = sec.strip_prefix("rule.") {
            if id.trim().is_empty() {
                continue;
            }
            let name = kv.get("name").cloned().unwrap_or_else(|| id.to_string());
            let kind = match kv.get("kind").map(|s| s.to_ascii_lowercase()) {
                Some(k) if k == "platform" => RuleKind::Platform,
                _ => RuleKind::Game,
            };
            let enabled = get_bool(kv, "enabled", true);
            let volume = get_u64(kv, "volume", 15).min(100) as u8;
            // processes = "a.exe;b.exe|C:\\path\\x.exe" → name;name|path 混合列表
            let mut processes = Vec::new();
            if let Some(list) = kv.get("processes") {
                for item in list.split(';') {
                    let item = item.trim();
                    if item.is_empty() {
                        continue;
                    }
                    if let Some((name, path)) = item.split_once('|') {
                        if !name.trim().is_empty() {
                            processes.push(ProcessPattern {
                                name: name.trim().to_string(),
                                path: path.trim().to_string(),
                            });
                        }
                    } else {
                        processes.push(ProcessPattern {
                            name: item.to_string(),
                            path: String::new(),
                        });
                    }
                }
            }
            rules.push(Rule {
                id: id.to_string(),
                name,
                kind,
                enabled,
                volume,
                processes,
            });
        }
    }

    let cfg = Config {
        version: 1,
        server_url,
        token,
        speakers,
        normal_volume,
        match_mode,
        restore_mode,
        check_interval,
        exit_debounce,
        // 与默认一致：导入后仍默认启用监控，保存后立即生效
        start_paused: false,
        auto_start: false,
        rules,
    };
    cfg.validate()?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
; SoundPilot legacy config
[general]
server_url = https://xiaoi.example.com
token = legacysecret
normal_volume = 30
check_interval = 5
exit_debounce = 10
match_mode = order
restore_mode = previous

[speaker.123456]
name = 客厅音箱
enabled = true

[speaker.999]
name = 卧室
enabled = false

[rule.r1]
name = 原神
kind = game
enabled = true
volume = 15
processes = YuanShen.exe;UnityWndClass

[rule.r2]
name = Steam 平台
kind = platform
enabled = false
volume = 25
processes = steam.exe|C:\Program Files (x86)\Steam\steam.exe
"#;

    #[test]
    fn parses_ini_sections() {
        let cfg = parse_ini_config(SAMPLE).unwrap();
        assert_eq!(cfg.server_url, "https://xiaoi.example.com");
        assert_eq!(cfg.token, "legacysecret");
        assert_eq!(cfg.speakers.len(), 2);
        assert_eq!(cfg.speakers[0].did, "123456");
        assert_eq!(cfg.speakers[0].name, "客厅音箱");
        assert!(!cfg.speakers[1].enabled);
        assert_eq!(cfg.rules.len(), 2);
        assert_eq!(cfg.rules[0].name, "原神");
        assert_eq!(cfg.rules[0].kind, RuleKind::Game);
        assert_eq!(cfg.rules[0].volume, 15);
        assert_eq!(cfg.rules[0].processes.len(), 2);
        assert_eq!(cfg.rules[1].kind, RuleKind::Platform);
        assert!(!cfg.rules[1].enabled);
        assert_eq!(
            cfg.rules[1].processes[0],
            ProcessPattern {
                name: "steam.exe".into(),
                path: "C:\\Program Files (x86)\\Steam\\steam.exe".into()
            }
        );
        assert_eq!(cfg.restore_mode, RestoreMode::Previous);
        assert!(!cfg.start_paused, "导入后默认启用监控 startPaused=false");
    }

    #[test]
    fn ini_comments_and_case() {
        let ini = parse_ini("[Gen]\nKey = Value ; trailing\n#comment\n");
        assert_eq!(ini["gen"]["key"], "Value ; trailing");
    }

    #[test]
    fn ini_bad_values_fall_back() {
        let cfg = parse_ini_config("[general]\nnormal_volume = abc\ncheck_interval = 0\n").unwrap();
        assert_eq!(cfg.normal_volume, 30);
        assert_eq!(cfg.check_interval, 1);
        assert!(cfg.speakers.is_empty());
    }
}
