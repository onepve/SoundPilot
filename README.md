# 小爱音量助手 · SoundPilot

Windows 游戏与平台音量规则助手。Rust / Tauri 2 / Vue 3。通过 xiaoi Webhook 控制音箱，非音频驱动，不降低蓝牙延迟。

## 配套项目（两者搭配使用）

SoundPilot **不直接与音箱通信**，需要搭配小爱音箱语音通知工具 **xiaoi** 一起使用：

| 角色 | 项目 | 说明 |
| :--- | :--- | :--- |
| 音箱通信 + Webhook 服务 | [**onepve/xiaoi**](https://github.com/onepve/xiaoi) | 本工具直接对接的版本：在上游基础上修复内存泄漏，并新增 **Webhook 与 CLI 获取音箱实时音量**（SoundPilot 写入后读回核验所需） |
| 桌面端规则控制 | **SoundPilot**（本仓库） | 检测游戏/平台进程，按规则调用 Webhook 调整音量 |
| 上游原作者 | [xvhuan/xiaoi](https://github.com/xvhuan/xiaoi) | 感谢原作者，本项目在其基础上复刻修改 |

搭配流程：先部署 [onepve/xiaoi](https://github.com/onepve/xiaoi) 并启用 Webhook（拿到地址与 Token）→ 在 SoundPilot 的「设置」中填入该地址与 Token、选择音箱 → 在「游戏与平台」页配置规则。

## 测试与构建

GitHub Actions `Windows test build`：前端测试、Rust 单元测试、Tauri Windows x64 构建、Windows 原生窗口启动检查。构建产物仅 Actions artifact，不发布 Release。

## 配置安全

`soundpilot.json` 与 EXE 放同一可写目录。首次配置**默认启用监控**（startPaused=false）；未配置音箱/服务器时不会发送任何音量请求。真实服务器地址、音箱 ID、Token 仅本机私密文件保存，不得提交仓库。导出的共享规则应移除 Token。

## 运行要求

Windows 10/11 x64，Microsoft Edge WebView2 Runtime。EXE 尚未代码签名，可能出现 SmartScreen 提示；仅在确认文件来源与哈希后决定是否运行，不建议关闭系统安全防护。

## 使用边界

不要同时运行旧 Go 监控程序和本工具，避免音量互相覆盖。暂停仅停止本客户端自动调音量，不会停止音箱播放或关闭其他自动化。网络写入已经抵达音箱后无法撤销，但暂停应阻止后续旧指令与重试。强制结束进程或断电无法保证恢复音量。
