# SynapCore Provider

> **crate名**: `synapcore_provider`  
> **定位**: Core 的扩展层，提供定时任务、统一消息发送、系统通知能力  
> **依赖**: `synapcore_core`, `tools`, `tokio`, `notify-rust`, `serde`, `serde_json`, `chrono`, `regex`, `uuid`, `dirs`, `thiserror`

---

## 架构

```
provider/src
├── lib.rs                  # Provider 主入口
├── timer/
│   ├── mod.rs              # Timer / TimerStore / TimerLoop / TimerNotification
│   └── README.md           # timer 模块详细文档
└── notify/
    └── mod.rs              # SystemNotify (notify-rust 封装)
```

---

## Provider

```rust
use synapcore_provider::Provider;

let mut provider = Provider::new()?;   // 初始化 Core + shutdown 通道

// 统一消息发送
let rx = provider.send(&message).await?;

// 启动主循环（TimerLoop + 系统通知）
provider.run().await?;
```

### send()

根据 `UserMessage.mode` 分发：

| mode | 调用 |
|------|------|
| `SendMode::Task` | `core.task(message)` |
| `SendMode::Chat` | `core.chat(&character, message)` |

### run()

1. 初始化 `TimerLoop`（独立 Core 实例 + shutdown 订阅）
2. `tokio::spawn` 运行 TimerLoop
3. 主循环 `tokio::select!`：
   - 接收 `TimerNotification` → `SystemNotify::send()` 发送桌面通知
   - 检测 shutdown 信号 → 退出

---

## timer 模块

参见 `src/timer/README.md`。

核心组件：

| 组件 | 职责 |
|------|------|
| `Timer` | 定时任务数据结构 (id/time/character/prompt/done) |
| `TimerStore` | timer.json 读写层 |
| `TimerLoop` | 30s 轮询 + fire + 发送 TimerNotification |
| `TimerNotification` | fire 结果 (character + body)，传递给 Provider 主循环 |

数据流：

```
timer.json ←→ TimerStore ← TimerLoop.check_and_fire()
                                ↓ fire()
                           Core.chat() → <timer>content</timer>
                                ↓ extract_tag_content + truncate
                           TimerNotification → mpsc → Provider
                                                       ↓
                                                 SystemNotify::send()
```

---

## notify 模块

```rust
use synapcore_provider::SystemNotify;

SystemNotify::send("标题", "内容")?;  // 调用 notify-rust 发送桌面通知
```

错误类型：`NotifyErr::Send(String)`

---

## 对其他 crate 的修改

### synapcore_core

- `UserMessage` 新增 `mode: SendMode` 和 `character: String` 字段
- 新增 `SendMode` 枚举 (`Task` / `Chat`)
- 新增 `UserMessage::task(text)` 和 `UserMessage::chat(character)` 工厂方法
- `pub use error::{CoreErr, CoreResult}` 重新导出

### synapcore_tools

- 新增 `timer` Inner 工具 (`tools/src/timer/mod.rs`)
  - `add`: 添加定时任务 (time/character/prompt)
  - `list`: 列出未完成任务
  - `remove`: 按 ID 删除任务
- `ToolResponse` 新增 `Timer { action, content }` 变体
- `Tools::default()` 注册 timer Inner 条目
- `Tools::call()` 和 `get_enabled_inner()` 注册 timer 分支

---

## 存储

| 文件 | 路径 |
|------|------|
| timer.json | `~/.cache/synapcore_cache/timer.json` |
