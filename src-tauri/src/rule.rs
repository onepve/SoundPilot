//! 规则引擎：进程快照 → 命中规则与目标音量；无规则时回落决策。
//!
//! 契约（plan §4）：优先游戏(kind=game)；同组内按 matchMode（order=数组顺序 / volume=最高音量）；
//! path 非空时精准路径匹配（不区分大小写/分隔符），否则名称不区分大小写。
//! 决策只在「规则身份」或「目标音量」变化时产生写指令（SameRule 不重复写）。

use crate::config::{Config, MatchMode, Rule};
use crate::process::ProcessEntry;

/// 一次评估的决策结果（纯函数，不发 IO）。
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// 暂停监控（含定时暂停）——不产生任何音量写
    Paused,
    /// 命中规则：rule id、名称、目标音量（写仅在 rule_identity/target 变化时）
    RuleHit {
        rule_id: String,
        rule_name: String,
        volume: u8,
    },
    /// 无规则命中：按 restoreMode 回落（normal/previous/none）
    NoRule {
        restore: RestoreAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreAction {
    /// 写 normalVolume
    ToNormal,
    /// 写启动时备份的每音箱原音量
    ToPrevious,
    /// 不写
    Skip,
}

/// 在候选规则中选择：游戏优先，同 kind 内按 matchMode。
/// 返回 Some(规则) 或 None。`rules` 顺序即配置数组顺序。
pub fn select_rule<'a>(
    rules: &'a [Rule],
    entry: &dyn Fn(&Rule) -> bool,
    match_mode: MatchMode,
) -> Option<&'a Rule> {
    let matched: Vec<&Rule> = rules.iter().filter(|r| entry(r)).collect();
    select_from_matched(matched, match_mode)
}

fn select_from_matched(matched: Vec<&Rule>, match_mode: MatchMode) -> Option<&Rule> {
    if matched.is_empty() {
        return None;
    }
    // 游戏优先：只看游戏组；若全为平台则看平台组
    let group: Vec<&&Rule> = matched
        .iter()
        .filter(|r| r.kind == crate::config::RuleKind::Game)
        .collect();
    let group: Vec<&&Rule> = if group.is_empty() {
        matched.iter().collect()
    } else {
        group
    };
    match match_mode {
        MatchMode::Order => group.first().copied().copied().map(|r| r),
        MatchMode::Volume => group
            .iter()
            .max_by_key(|r| r.volume)
            .copied()
            .copied(),
    }
}

/// 进程是否命中规则的某个 pattern。
pub fn rule_matches_process(rule: &Rule, proc_entry: &ProcessEntry) -> bool {
    rule.processes.iter().any(|p| {
        if p.path.is_empty() {
            names_equal(&p.name, &proc_entry.name)
        } else {
            paths_equal(&p.path, &proc_entry.path)
        }
    })
}

/// 名称匹配：不区分大小写，比较去路径的文件名。
pub fn names_equal(a: &str, b: &str) -> bool {
    file_name_of(a).eq_ignore_ascii_case(&file_name_of(b))
}

/// 路径匹配：不区分大小写、统一 `/`与`\`。
pub fn paths_equal(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").to_ascii_lowercase();
    // 允许配置写完整路径而快照只有名字（或反之）时按文件名兜底? 不——path 非空必须精准路径匹配
    norm(a) == norm(b)
}

fn file_name_of(s: &str) -> &str {
    let s = s.trim();
    match s.rfind(['\\', '/']) {
        Some(i) => &s[i + 1..],
        None => s,
    }
}

/// 评估当前快照 → 决策（含暂停判断由调用方先行处理）。
pub fn evaluate(config: &Config, processes: &[ProcessEntry]) -> Decision {
    let matched = config
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter(|r| processes.iter().any(|pe| rule_matches_process(r, pe)))
        .collect::<Vec<_>>();
    match select_from_matched(matched, config.match_mode) {
        Some(rule) => Decision::RuleHit {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            volume: rule.volume,
        },
        None => Decision::NoRule {
            restore: match config.restore_mode {
                crate::config::RestoreMode::Normal => RestoreAction::ToNormal,
                crate::config::RestoreMode::Previous => RestoreAction::ToPrevious,
                crate::config::RestoreMode::None => RestoreAction::Skip,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Config, MatchMode, ProcessPattern, RestoreMode, Rule, RuleKind,
    };

    fn rule(id: &str, name: &str, kind: RuleKind, volume: u8, processes: &[(&str, &str)]) -> Rule {
        Rule {
            id: id.into(),
            name: name.into(),
            kind,
            enabled: true,
            volume,
            processes: processes
                .iter()
                .map(|(n, p)| ProcessPattern {
                    name: n.to_string(),
                    path: p.to_string(),
                })
                .collect(),
        }
    }

    fn proc(name: &str, path: &str) -> ProcessEntry {
        ProcessEntry {
            pid: 1,
            name: name.into(),
            path: path.into(),
            title: String::new(),
            has_window: true,
        }
    }

    fn cfg(rules: Vec<Rule>, match_mode: MatchMode, restore: RestoreMode) -> Config {
        Config {
            version: 1,
            server_url: String::new(),
            token: String::new(),
            speakers: vec![],
            normal_volume: 30,
            match_mode,
            restore_mode: restore,
            check_interval: 5,
            exit_debounce: 10,
            start_paused: false,
            auto_start: false,
            rules,
        }
    }

    #[test]
    fn game_overrides_platform() {
        let c = cfg(
            vec![
                rule("p1", "Steam", RuleKind::Platform, 25, &[("steam.exe", "")]),
                rule("g1", "原神", RuleKind::Game, 15, &[("yuanshen.exe", "")]),
            ],
            MatchMode::Order,
            RestoreMode::Normal,
        );
        let procs = vec![proc("steam.exe", ""), proc("YuanShen.exe", "")];
        match evaluate(&c, &procs) {
            Decision::RuleHit { rule_id, volume, .. } => {
                assert_eq!(rule_id, "g1", "游戏必须覆盖平台");
                assert_eq!(volume, 15);
            }
            other => panic!("expected RuleHit, got {other:?}"),
        }
    }

    #[test]
    fn order_mode_first_in_array() {
        let c = cfg(
            vec![
                rule("r-first", "先", RuleKind::Game, 20, &[("a.exe", "")]),
                rule("r-second", "后", RuleKind::Game, 40, &[("a.exe", "")]),
            ],
            MatchMode::Order,
            RestoreMode::Normal,
        );
        let procs = vec![proc("a.exe", "")];
        match evaluate(&c, &procs) {
            Decision::RuleHit { rule_id, volume, .. } => {
                assert_eq!(rule_id, "r-first");
                assert_eq!(volume, 20);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn volume_mode_highest_volume() {
        let c = cfg(
            vec![
                rule("low", "低", RuleKind::Game, 10, &[("a.exe", "")]),
                rule("high", "高", RuleKind::Game, 45, &[("a.exe", "")]),
            ],
            MatchMode::Volume,
            RestoreMode::Normal,
        );
        let procs = vec![proc("a.exe", "")];
        match evaluate(&c, &procs) {
            Decision::RuleHit { rule_id, volume, .. } => {
                assert_eq!(rule_id, "high");
                assert_eq!(volume, 45);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn disabled_rules_skipped() {
        let mut r = rule("g1", "游戏", RuleKind::Game, 15, &[("a.exe", "")]);
        r.enabled = false;
        let c = cfg(vec![r], MatchMode::Order, RestoreMode::None);
        match evaluate(&c, &[proc("a.exe", "")]) {
            Decision::NoRule { restore: RestoreAction::Skip } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn path_exact_matching_case_and_slash_insensitive() {
        let c = cfg(
            vec![rule(
                "p",
                "路径",
                RuleKind::Game,
                12,
                &[("steam.exe", "C:/Program Files (x86)/Steam/steam.exe")],
            )],
            MatchMode::Order,
            RestoreMode::None,
        );
        // 大小写与反斜杠差异仍命中
        let hit = vec![proc(
            "steam.exe",
            "c:\\program files (x86)\\steam\\Steam.exe",
        )];
        assert!(matches!(
            evaluate(&c, &hit),
            Decision::RuleHit { .. }
        ));
        // 路径不同（仅文件名相同）不命中
        let miss = vec![proc("steam.exe", "D:\\other\\steam.exe")];
        assert!(matches!(
            evaluate(&c, &miss),
            Decision::NoRule { .. }
        ));
    }

    #[test]
    fn name_match_case_insensitive_including_path_in_pattern_name() {
        let c = cfg(
            vec![rule("n", "名称", RuleKind::Game, 18, &[("YuAnShEn.EXE", "")])],
            MatchMode::Order,
            RestoreMode::None,
        );
        assert!(matches!(
            evaluate(&c, &[proc("yuanshen.exe", "C:\\x\\yuanshen.exe")]),
            Decision::RuleHit { .. }
        ));
    }

    #[test]
    fn no_rule_fallback_modes() {
        let mk = |rm| cfg(vec![], MatchMode::Order, rm);
        assert!(matches!(
            evaluate(&mk(RestoreMode::Normal), &[]),
            Decision::NoRule { restore: RestoreAction::ToNormal }
        ));
        assert!(matches!(
            evaluate(&mk(RestoreMode::Previous), &[]),
            Decision::NoRule { restore: RestoreAction::ToPrevious }
        ));
        assert!(matches!(
            evaluate(&mk(RestoreMode::None), &[]),
            Decision::NoRule { restore: RestoreAction::Skip }
        ));
    }

    #[test]
    fn platform_only_when_no_game_running() {
        let c = cfg(
            vec![
                rule("g", "游戏", RuleKind::Game, 15, &[("game.exe", "")]),
                rule("p", "平台", RuleKind::Platform, 25, &[("steam.exe", "")]),
            ],
            MatchMode::Order,
            RestoreMode::Normal,
        );
        let procs = vec![proc("STEAM.EXE", "")];
        match evaluate(&c, &procs) {
            Decision::RuleHit { rule_id, volume, .. } => {
                assert_eq!(rule_id, "p");
                assert_eq!(volume, 25);
            }
            other => panic!("{other:?}"),
        }
    }
}

