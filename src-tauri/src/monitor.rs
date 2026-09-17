//! 串行监控 actor：单任务循环，所有音箱写操作排队执行，保证
//! 「暂停/配置变更后，旧的音量任务不得再发送」（plan §8）。
//!
//! 设计：`MonitorHandle` 持有 unbounded sender；唯一的 actor 任务持有 receiver
//! 与 sender 克隆（用于把检测产生的写重新入队，保持严格 FIFO 串行）。
//! 每条写消息携带创建时的 generation，actor 发送 HTTP 前比对当前
//! generation —— 不匹配（暂停/配置变更/退出）则丢弃，杜绝陈旧写。
//! 手动音量（set_manual_volume）同样经 actor 串行执行：先暂停（bump），
//! 再以新 generation 入队 ManualVolume，因此不会被暂停检查丢弃，
//! 也不会与任何自动写交错。Stop 消息排在队列尾部：之前的写要么已完成
//! 要么已按 generation 丢弃，退出与写天然串行。

use crate::process::ProcessEntry;
use crate::rule::{evaluate, Decision, RestoreAction};
use crate::state::{AppState, Generation, Status};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub enum MonitorMsg {
    /// 立即执行一轮检测（定时器/配置保存后触发）
    CheckNow,
    /// 自动写音量（执行前校验 generation；暂停时丢弃）
    ApplyVolume {
        generation: Generation,
        did: String,
        volume: u8,
        reason: String,
    },
    /// 手动写音量（暂停后仍执行——用户显式操作；仅校验 generation 与退出）
    ManualVolume {
        generation: Generation,
        did: Option<String>,
        volume: u8,
        reply: oneshot::Sender<Result<Status, String>>,
    },
    /// 退出前恢复音量（免 generation/暂停/shutdown 拦截；done 用于同步等待恢复完成）
    Restore {
        done: std::sync::mpsc::Sender<()>,
    },
    /// 停止 actor（退出）
    Stop,
}

#[derive(Clone)]
pub struct MonitorHandle {
    tx: mpsc::UnboundedSender<MonitorMsg>,
}

impl MonitorHandle {
    /// 生产入口：启动 actor + 定时检测调度。
    pub fn new(state: Arc<AppState>) -> Self {
        let snapshotter: Arc<dyn Fn() -> Vec<ProcessEntry> + Send + Sync> =
            Arc::new(crate::process::snapshot_entries);
        Self::with_parts(state, snapshotter, true)
    }

    /// 测试入口：注入快照函数，不启动定时调度。
    pub fn new_with_snapshotter(
        state: Arc<AppState>,
        snapshotter: Arc<dyn Fn() -> Vec<ProcessEntry> + Send + Sync>,
    ) -> Self {
        Self::with_parts(state, snapshotter, false)
    }

    fn with_parts(
        state: Arc<AppState>,
        snapshotter: Arc<dyn Fn() -> Vec<ProcessEntry> + Send + Sync>,
        schedule: bool,
    ) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = MonitorHandle { tx: tx.clone() };
        // tauri 全局异步运行时（cargo test 下亦可用）
        tauri::async_runtime::spawn(actor_loop(rx, tx, state.clone(), snapshotter));
        if schedule {
            let sched_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                while !state.is_shutting_down() {
                    let secs = {
                        let cfg = state.config().read().unwrap().clone();
                        cfg.check_interval.max(1)
                    };
                    tokio::time::sleep(Duration::from_secs(secs)).await;
                    if state.is_shutting_down() {
                        break;
                    }
                    let _ = sched_handle.tx.send(MonitorMsg::CheckNow);
                }
            });
        }
        handle
    }

    pub fn check_now(&self) {
        let _ = self.tx.send(MonitorMsg::CheckNow);
    }

    pub fn stop(&self) {
        let _ = self.tx.send(MonitorMsg::Stop);
    }

    /// 退出恢复：入队 Restore（FIFO 排在在途写之后）并等待恢复完成。
    /// actor 已停止时立即返回（无事可做）。
    /// 用 std 通道同步等待——tokio 上下文内 block_on 会 panic，std recv 任何线程都安全。
    pub fn restore_and_wait(&self, timeout: Duration) {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        if self
            .tx
            .send(MonitorMsg::Restore { done: done_tx })
            .is_err()
        {
            return; // actor 已退出，无恢复必要
        }
        let _ = done_rx.recv_timeout(timeout);
    }

    /// 测试专用：直接访问 actor 通道。
    #[cfg(test)]
    pub(crate) fn sender_for_test(&self) -> &mpsc::UnboundedSender<MonitorMsg> {
        &self.tx
    }

    /// 手动音量（调用方必须已 pause 使 generation 为最新）。
    pub async fn manual_volume(
        &self,
        generation: Generation,
        volume: u8,
        did: Option<String>,
    ) -> Result<Status, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(MonitorMsg::ManualVolume {
                generation,
                did,
                volume,
                reply: reply_tx,
            })
            .map_err(|_| "监控器已停止".to_string())?;
        match tokio::time::timeout(Duration::from_secs(30), reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("监控器已停止".to_string()),
            Err(_) => Err("手动设置超时".to_string()),
        }
    }
}

/// actor 主循环：串行处理消息（严格 FIFO，与退出串行）。
async fn actor_loop(
    mut rx: mpsc::UnboundedReceiver<MonitorMsg>,
    tx: mpsc::UnboundedSender<MonitorMsg>,
    state: Arc<AppState>,
    snapshotter: Arc<dyn Fn() -> Vec<ProcessEntry> + Send + Sync>,
) {
    while let Some(msg) = rx.recv().await {
        if state.is_shutting_down() {
            // 关停中只处理恢复请求，其余全部丢弃（防在途写闯关）
            if let MonitorMsg::Restore { done } = msg {
                restore_on_exit(&state).await;
                let _ = done.send(());
            }
            break;
        }
        match msg {
            MonitorMsg::Stop => break,
            MonitorMsg::Restore { done } => {
                restore_on_exit(&state).await;
                let _ = done.send(());
            }
            MonitorMsg::CheckNow => {
                if !state.is_paused() {
                    let snap: &(dyn Fn() -> Vec<ProcessEntry> + Sync) = snapshotter.as_ref();
                    run_check(&state, snap, &tx).await;
                }
            }
            MonitorMsg::ApplyVolume {
                generation,
                did,
                volume,
                reason,
            } => {
                // 关键：暂停/配置变更/退出后，旧任务不再发送 HTTP
                if generation != state.current_generation() || state.is_paused() {
                    state.log_event(
                        "debug",
                        &format!("丢弃过期音量任务（{reason}）: 代数 {generation} 已失效"),
                    );
                    continue;
                }
                apply_volume(&state, &did, volume, &reason).await;
            }
            MonitorMsg::ManualVolume {
                generation,
                did,
                volume,
                reply,
            } => {
                if generation != state.current_generation() || state.is_shutting_down() {
                    let _ = reply.send(Err("任务已失效（暂停/配置状态已变化），请重试".into()));
                    continue;
                }
                let result = manual_volume_impl(&state, volume, did).await;
                let _ = reply.send(result);
            }
        }
    }
}

/// 手动音量实现（仅 actor 调用，串行）：先暂停已由调用方完成。
async fn manual_volume_impl(
    state: &Arc<AppState>,
    volume: u8,
    did: Option<String>,
) -> Result<Status, String> {
    use crate::state::DeviceStatus;
    if volume > 100 {
        return Err("音量必须在 0..100".into());
    }
    let cfg = state.config().read().unwrap().clone();
    let targets: Vec<(String, String)> = cfg
        .speakers
        .iter()
        .filter(|s| match &did {
            Some(d) => &s.did == d,
            None => s.enabled,
        })
        .map(|s| (s.did.clone(), s.name.clone()))
        .collect();
    if targets.is_empty() {
        return Err("没有匹配的音箱".into());
    }
    let mut devices = Vec::new();
    for (speaker_did, name) in &targets {
        let client = crate::http_client::SpeakerClient::new(&cfg.server_url, &cfg.token);
        let dev = match client.set_volume(speaker_did, volume).await {
            Ok(v) => {
                state.log_event("info", &format!("手动设置 {name} 音量为 {v}"));
                DeviceStatus {
                    did: speaker_did.clone(),
                    name: name.clone(),
                    volume: Some(v),
                    error: None,
                }
            }
            Err(e) => {
                let msg = e.to_string();
                state.log_event("error", &format!("手动设置 {name} 失败: {msg}"));
                DeviceStatus {
                    did: speaker_did.clone(),
                    name: name.clone(),
                    volume: None,
                    error: Some(msg),
                }
            }
        };
        devices.push(dev);
    }
    state.update_devices(devices);
    Ok(state.status())
}

/// 执行一次自动写入 + 延时核验（读回）。仅由 actor 调用。
async fn apply_volume(state: &Arc<AppState>, did: &str, volume: u8, reason: &str) {
    let cfg = state.config().read().unwrap().clone();
    let client = crate::http_client::SpeakerClient::new(&cfg.server_url, &cfg.token);
    match client.set_volume(did, volume).await {
        Ok(v) => {
            state.log_event("info", &format!("{reason}: {did} 音量已设为 {v}"));
        }
        Err(e) => {
            state.log_event("error", &format!("{reason}: {did} 写入失败: {e}"));
        }
    }
    // 发送后核验：读回慢于写，延时后读一次（仅告警，不纠偏——plan §8 无同规则纠偏）
    tokio::time::sleep(Duration::from_millis(300)).await;
    if let Ok(actual) = client.get_volume(did).await {
        if actual != volume {
            state.log_event(
                "warn",
                &format!("{reason}: {did} 核验音量 {actual} != 目标 {volume}"),
            );
        }
    }
}

/// 退出/关机前的音量恢复（仅 actor 调用）：
/// 仅当有规则生效时才写；恢复语义与规则退出一致（RestoreMode）。
/// 使用短超时零重试客户端——退出路径不能被慢网络拖住。
async fn restore_on_exit(state: &Arc<AppState>) {
    let (rule_id, _, _) = state.active_rule.lock().unwrap().clone();
    if rule_id.is_none() {
        return; // 无规则生效，音箱本来就是用户音量，不动
    }
    let cfg = state.config().read().unwrap().clone();
    for s in cfg.speakers.iter().filter(|s| s.enabled) {
        let target = match cfg.restore_mode {
            crate::config::RestoreMode::Normal => Some(cfg.normal_volume),
            crate::config::RestoreMode::Previous => state.baseline(&s.did),
            crate::config::RestoreMode::None => None,
        };
        let Some(volume) = target else { continue };
        let client = crate::http_client::SpeakerClient::with_options(
            &cfg.server_url,
            &cfg.token,
            Duration::from_secs(1),
            0,
            Duration::from_millis(0),
        );
        match client.set_volume(&s.did, volume).await {
            Ok(v) => state.log_event("info", &format!("退出恢复: {} 音量已设为 {v}", s.name)),
            Err(e) => state.log_event("error", &format!("退出恢复: {} 写入失败: {e}", s.name)),
        }
    }
}

/// 一轮检测：快照 → 决策 → 状态更新 →（规则身份或目标变化时）入队写。
async fn run_check(
    state: &Arc<AppState>,
    // 需 + Sync：引用被 Send future 捕获，dyn Fn 不 Sync 会导致 actor_loop 非 Send
    snapshotter: &(dyn Fn() -> Vec<ProcessEntry> + Sync),
    tx: &mpsc::UnboundedSender<MonitorMsg>,
) {
    let cfg = state.config().read().unwrap().clone();
    let processes = snapshotter();
    let decision = evaluate(&cfg, &processes);
    decide_and_enqueue(state, &cfg, &decision, tx);
}

/// 决策落地：更新 activeRule 状态；仅在「规则身份/目标变化」或「从有规则退出」时入队写。
pub fn decide_and_enqueue(
    state: &Arc<AppState>,
    cfg: &crate::config::Config,
    decision: &Decision,
    tx: &mpsc::UnboundedSender<MonitorMsg>,
) {
    let generation = state.current_generation();
    match decision {
        Decision::Paused => { /* 调用方（actor）已过滤；保底不写 */ }
        Decision::RuleHit {
            rule_id,
            rule_name,
            volume,
        } => {
            let (prev_id, _, prev_target) = state.active_rule.lock().unwrap().clone();
            if prev_id.as_deref() != Some(rule_id.as_str()) || prev_target != Some(*volume) {
                state.set_active_rule(
                    Some(rule_id.clone()),
                    Some(rule_name.clone()),
                    Some(*volume),
                );
                state.log_event(
                    "info",
                    &format!("命中规则「{rule_name}」目标音量 {volume}"),
                );
                for s in cfg.speakers.iter().filter(|s| s.enabled) {
                    let _ = tx.send(MonitorMsg::ApplyVolume {
                        generation,
                        did: s.did.clone(),
                        volume: *volume,
                        reason: format!("规则「{rule_name}」"),
                    });
                }
            }
        }
        Decision::NoRule { restore } => {
            let (prev_id, _, _) = state.active_rule.lock().unwrap().clone();
            if prev_id.is_some() {
                state.set_active_rule(None, None, None);
                match restore {
                    RestoreAction::ToNormal => {
                        state.log_event("info", "规则退出，恢复正常音量");
                        for s in cfg.speakers.iter().filter(|s| s.enabled) {
                            let _ = tx.send(MonitorMsg::ApplyVolume {
                                generation,
                                did: s.did.clone(),
                                volume: cfg.normal_volume,
                                reason: "恢复正常音量".to_string(),
                            });
                        }
                    }
                    RestoreAction::ToPrevious => {
                        state.log_event("info", "规则退出，恢复原音量");
                        for s in cfg.speakers.iter().filter(|s| s.enabled) {
                            if let Some(prev) = state.baseline(&s.did) {
                                let _ = tx.send(MonitorMsg::ApplyVolume {
                                    generation,
                                    did: s.did.clone(),
                                    volume: prev,
                                    reason: "恢复原音量".to_string(),
                                });
                            }
                        }
                    }
                    RestoreAction::Skip => {
                        state.log_event("info", "规则退出（不恢复音量）");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Config, MatchMode, ProcessPattern, RestoreMode, Rule, RuleKind, Speaker,
    };

    fn mk_config(rules: Vec<Rule>, restore: RestoreMode) -> Config {
        Config {
            version: 1,
            server_url: String::new(),
            token: String::new(),
            speakers: vec![Speaker {
                did: "d1".into(),
                name: "S".into(),
                enabled: true,
            }],
            normal_volume: 40,
            match_mode: MatchMode::Order,
            restore_mode: restore,
            check_interval: 5,
            exit_debounce: 10,
            start_paused: true,
            auto_start: false,
            rules,
        }
    }

    fn game_rule() -> Rule {
        Rule {
            id: "g1".into(),
            name: "游戏".into(),
            kind: RuleKind::Game,
            enabled: true,
            volume: 15,
            processes: vec![ProcessPattern {
                name: "game.exe".into(),
                path: String::new(),
            }],
        }
    }

    fn entry(name: &str) -> ProcessEntry {
        ProcessEntry {
            pid: 1,
            name: name.into(),
            path: String::new(),
            title: String::new(),
            has_window: true,
        }
    }

    #[tokio::test]
    async fn same_rule_does_not_requeue_write() {
        let state = Arc::new(AppState::new(mk_config(vec![game_rule()], RestoreMode::Normal)));
        state.resume_monitor();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let snap = || vec![entry("game.exe")];
        run_check(&state, &snap, &tx).await;
        match rx.try_recv() {
            Ok(MonitorMsg::ApplyVolume { volume, did, .. }) => {
                assert_eq!(volume, 15);
                assert_eq!(did, "d1");
            }
            other => panic!("expected first write, got {other:?}"),
        }
        // 相同规则第二轮：不重复写
        run_check(&state, &snap, &tx).await;
        assert!(rx.try_recv().is_err(), "相同规则不应重复入队");
    }

    #[tokio::test]
    async fn target_volume_change_requeues() {
        let state = Arc::new(AppState::new(mk_config(vec![game_rule()], RestoreMode::Normal)));
        state.resume_monitor();
        let (tx, mut rx) = mpsc::unbounded_channel();
        run_check(&state, &|| vec![entry("game.exe")], &tx).await;
        let _ = rx.try_recv();
        // 用户改了规则音量（保存配置 15→20；仅内存生效，避免测试写盘）
        let mut cfg2 = mk_config(vec![game_rule()], RestoreMode::Normal);
        cfg2.rules[0].volume = 20;
        state.apply_config_in_memory(cfg2);
        run_check(&state, &|| vec![entry("game.exe")], &tx).await;
        match rx.try_recv() {
            Ok(MonitorMsg::ApplyVolume { volume, .. }) => assert_eq!(volume, 20),
            other => panic!("expected re-write on new target, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rule_exit_restores_normal_then_quiet() {
        let state = Arc::new(AppState::new(mk_config(vec![game_rule()], RestoreMode::Normal)));
        state.resume_monitor();
        let (tx, mut rx) = mpsc::unbounded_channel();
        run_check(&state, &|| vec![entry("game.exe")], &tx).await;
        let _ = rx.try_recv();
        run_check(&state, &|| vec![], &tx).await;
        match rx.try_recv() {
            Ok(MonitorMsg::ApplyVolume { volume, reason, .. }) => {
                assert_eq!(volume, 40);
                assert!(reason.contains("正常"));
            }
            other => panic!("expected restore write, got {other:?}"),
        }
        // 持续无规则：不再写
        run_check(&state, &|| vec![], &tx).await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn rule_exit_restores_previous_baseline() {
        let state = Arc::new(AppState::new(mk_config(vec![game_rule()], RestoreMode::Previous)));
        state.set_baseline("d1", 55);
        state.resume_monitor();
        let (tx, mut rx) = mpsc::unbounded_channel();
        run_check(&state, &|| vec![entry("game.exe")], &tx).await;
        let _ = rx.try_recv();
        run_check(&state, &|| vec![], &tx).await;
        match rx.try_recv() {
            Ok(MonitorMsg::ApplyVolume { volume, reason, .. }) => {
                assert_eq!(volume, 55, "应恢复备份原音量");
                assert!(reason.contains("原音量"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn paused_actor_skips_check_entirely() {
        // 该用例专门验证暂停语义：显式构造 startPaused=true 的配置
        let mut cfg = mk_config(vec![game_rule()], RestoreMode::Normal);
        cfg.start_paused = true;
        let state = Arc::new(AppState::new(cfg));
        assert!(state.is_paused(), "startPaused 配置初始即暂停");
        let handle = MonitorHandle::new_with_snapshotter(
            state.clone(),
            Arc::new(|| vec![entry("game.exe")]),
        );
        handle.check_now();
        tokio::time::sleep(Duration::from_millis(150)).await;
        let status = state.status();
        assert!(status.paused, "未恢复前应保持暂停");
        assert_eq!(status.active_rule_id, None, "暂停期间不得识别规则");
        let events = state.events.lock().unwrap();
        assert!(
            !events.iter().any(|e| e.message.contains("命中规则")),
            "暂停期间不得产生规则写入: {:?}",
            events.iter().map(|e| e.message.clone()).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn stale_generation_task_is_dropped_not_sent() {
        let state = Arc::new(AppState::new(mk_config(vec![], RestoreMode::None)));
        state.resume_monitor();
        let handle = MonitorHandle::new_with_snapshotter(state.clone(), Arc::new(|| vec![]));

        // 先取旧代数，再暂停（bump），随后入队的旧任务必然已失效（无竞态）
        let old_gen = state.current_generation();
        state.pause_monitor(None);
        handle
            .sender_for_test()
            .send(MonitorMsg::ApplyVolume {
                generation: old_gen,
                did: "d1".into(),
                volume: 10,
                reason: "旧任务".into(),
            })
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let events = state.events.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.message.contains("丢弃过期音量任务")),
            "旧代数任务必须被丢弃，实际事件: {:?}",
            events.iter().map(|e| e.message.clone()).collect::<Vec<_>>()
        );
        assert!(
            !events.iter().any(|e| e.message.contains("写入失败")),
            "被丢弃的任务不得尝试 HTTP"
        );
    }

    #[tokio::test]
    async fn stop_message_terminates_actor() {
        let state = Arc::new(AppState::new(mk_config(vec![], RestoreMode::None)));
        state.resume_monitor();
        let handle = MonitorHandle::new_with_snapshotter(state.clone(), Arc::new(|| vec![]));
        handle.stop();
        // 通道关闭后 manual_volume 返回错误
        tokio::time::sleep(Duration::from_millis(200)).await;
        let result = handle.manual_volume(1, 10, None).await;
        assert!(result.is_err());
    }

    /// 手动音量走 mock HTTP：先暂停（安全），再经 actor 串行写入。
    #[tokio::test]
    async fn manual_volume_writes_via_mock_http_after_pause() {
        use crate::http_client::mock_server::{MockAction, MockServer};

        let server = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":25,"result":"ok"}"#.into(),
        )])
        .await;

        let mut cfg = mk_config(vec![], RestoreMode::None);
        cfg.server_url = server.url();
        cfg.token = "manual-test-token".into();
        let state = Arc::new(AppState::new(cfg));
        let handle = MonitorHandle::new_with_snapshotter(state.clone(), Arc::new(Vec::new));

        // 命令层等价流程：先暂停，再取新代数发手动写
        state.pause_monitor(None);
        let gen = state.current_generation();
        let status = handle
            .manual_volume(gen, 25, Some("d1".to_string()))
            .await
            .expect("手动音量应成功");
        assert!(status.paused, "手动设置后应处于暂停");
        assert_eq!(
            status.devices.iter().find(|d| d.did == "d1").and_then(|d| d.volume),
            Some(25)
        );

        // mock 收到的请求校验
        let reqs = server.recorded();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].method, "POST");
        assert!(reqs[0].body.contains(r#""did":"d1""#));
        assert!(reqs[0].body.contains(r#""volume":25"#));
        assert_eq!(reqs[0].token_header.as_deref(), Some("manual-test-token"));
    }

    /// 手动音量带旧代数 → 拒绝执行（防陈旧写同样覆盖手动路径）。
    #[tokio::test]
    async fn manual_volume_with_stale_generation_rejected() {
        let state = Arc::new(AppState::new(mk_config(vec![], RestoreMode::None)));
        state.resume_monitor();
        let handle = MonitorHandle::new_with_snapshotter(state.clone(), Arc::new(Vec::new));
        let old_gen = state.current_generation();
        state.pause_monitor(None); // bump → old_gen 失效
        let err = handle
            .manual_volume(old_gen, 30, None)
            .await
            .expect_err("旧代数手动任务必须被拒绝");
        assert!(err.contains("失效"), "{err}");
    }

    /// 退出恢复（Normal 模式）：规则生效时 Stop 前恢复 normalVolume，走真实 HTTP。
    #[tokio::test]
    async fn exit_restore_writes_normal_volume_via_mock_http() {
        use crate::http_client::mock_server::{MockAction, MockServer};

        let server = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":40,"result":"ok"}"#.into(),
        )])
        .await;

        let mut cfg = mk_config(vec![game_rule()], RestoreMode::Normal);
        cfg.server_url = server.url();
        cfg.token = "exit-restore-token".into();
        let state = Arc::new(AppState::new(cfg));
        state.resume_monitor();

        // 模拟规则生效：命中游戏规则（会写 15）
        let (tx, mut rx) = mpsc::unbounded_channel();
        run_check(&state, &|| vec![entry("game.exe")], &tx).await;
        let _ = rx.try_recv(); // 消费掉规则写入（不入队执行）
        assert!(state.active_rule.lock().unwrap().0.is_some(), "规则应已生效");

        // 退出恢复：应写 normalVolume=40
        restore_on_exit(&state).await;
        let reqs = server.recorded();
        assert_eq!(reqs.len(), 1, "退出恢复应恰好发一次写: {:?}", reqs);
        assert!(reqs[0].body.contains(r#""volume":40"#), "{}", reqs[0].body);
        assert_eq!(reqs[0].token_header.as_deref(), Some("exit-restore-token"));
    }

    /// 退出恢复（Previous 模式）：恢复启动备份的原音量。
    #[tokio::test]
    async fn exit_restore_uses_baseline_in_previous_mode() {
        use crate::http_client::mock_server::{MockAction, MockServer};

        let server = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":55,"result":"ok"}"#.into(),
        )])
        .await;

        let mut cfg = mk_config(vec![game_rule()], RestoreMode::Previous);
        cfg.server_url = server.url();
        let state = Arc::new(AppState::new(cfg));
        state.set_baseline("d1", 55);
        state.resume_monitor();
        state.set_active_rule(Some("g1".into()), Some("游戏".into()), Some(15));

        restore_on_exit(&state).await;
        let reqs = server.recorded();
        assert_eq!(reqs.len(), 1);
        assert!(reqs[0].body.contains(r#""volume":55"#), "{}", reqs[0].body);
    }

    /// 退出恢复：无规则生效时不发任何写（音箱本来就是用户音量）。
    #[tokio::test]
    async fn exit_restore_skips_when_no_rule_active() {
        use crate::http_client::mock_server::{MockAction, MockServer};

        let server = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":40,"result":"ok"}"#.into(),
        )])
        .await;

        let mut cfg = mk_config(vec![game_rule()], RestoreMode::Normal);
        cfg.server_url = server.url();
        let state = Arc::new(AppState::new(cfg));
        state.resume_monitor();
        assert!(state.active_rule.lock().unwrap().0.is_none());

        restore_on_exit(&state).await;
        assert!(server.recorded().is_empty(), "无规则不得发写请求");
    }

    /// Restore 消息穿透 shutdown 闸门：begin_shutdown 后 Restore 仍被执行（关键回归）。
    #[tokio::test]
    async fn restore_message_works_after_begin_shutdown() {
        use crate::http_client::mock_server::{MockAction, MockServer};

        let server = MockServer::start(vec![MockAction::Respond(
            200,
            r#"{"success":true,"volume":40,"result":"ok"}"#.into(),
        )])
        .await;

        let mut cfg = mk_config(vec![game_rule()], RestoreMode::Normal);
        cfg.server_url = server.url();
        let state = Arc::new(AppState::new(cfg));
        state.resume_monitor();
        state.set_active_rule(Some("g1".into()), Some("游戏".into()), Some(15));

        let handle = MonitorHandle::new_with_snapshotter(state.clone(), Arc::new(Vec::new));
        // 先标记关停（模拟退出路径已开始），Restore 仍必须被执行。
        // 不能直接调 restore_and_wait：std 阻塞会锁死 #[tokio::test] 单线程运行时，
        // mock 服务器无法推进；这里用异步轮询等完成，覆盖同一段 actor 逻辑。
        state.begin_shutdown();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        handle
            .sender_for_test()
            .send(MonitorMsg::Restore { done: done_tx })
            .unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while done_rx.try_recv().is_err() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "shutdown 后恢复超时未完成"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let reqs = server.recorded();
        assert_eq!(
            reqs.len(),
            1,
            "shutdown 后 Restore 必须仍能写恢复音量: {:?}",
            reqs
        );
        assert!(reqs[0].body.contains(r#""volume":40"#), "{}", reqs[0].body);
    }
}
