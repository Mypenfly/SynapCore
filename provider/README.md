# SynapCore Provider

> **crate名**: `synapcore_provider`  
> **定位**: Core 的扩展层，提供定时任务、统一消息发送、系统通知、自动进化循环能力  
> **依赖**: `synapcore_core`, `tools`, `tokio`, `notify-rust`, `serde`, `serde_json`, `chrono`, `regex`, `uuid`, `dirs`, `thiserror`

---

## 架构

```
src/
├── lib.rs                  # Provider 主入口 + 主循环
├── provider_cmd.rs         # ProviderCommand / ProviderResponse 定义
├── timer/
│   ├── mod.rs              # Timer / TimerStore / TimerLoop / TimerNotification
│   └── README.md           # timer 模块详细文档
├── auto_loop/
│   ├── mod.rs              # AutoLoopManager（调度器 + 持久化计时）
│   ├── auto.rs             # AutoLoop（AutoStudy / AutoReflect / AutoClear 执行逻辑）
│   └── README.md           # auto_loop 模块详细文档
└── notify/
    └── mod.rs              # SystemNotify (notify-rust 封装)
```

---

## Provider 主入口

### 初始化与基本使用

```rust
use synapcore_provider::{ProviderCommand, ProviderResponse, SystemNotify};
use synapcore_core::UserMessage;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = Provider::new()?;

    let (cmd_tx, cmd_rx) = mpsc::channel::<ProviderCommand>(1024);
    let (resp_tx, mut resp_rx) = mpsc::channel::<ProviderResponse>(1024);

    let provider_handle = tokio::spawn(async move {
        if let Err(e) = provider.run(cmd_rx, resp_tx).await {
            eprintln!("Provider 运行失败: {}", e);
        }
    });

    let message = UserMessage::task("你好");
    cmd_tx.send(ProviderCommand::Send { message }).await?;

    while let Some(resp) = resp_rx.recv().await {
        match resp {
            ProviderResponse::Response(bot_resp) => println!("{}", bot_resp),
            ProviderResponse::Error(err) => eprintln!("错误: {}", err),
        }
    }

    let _ = provider_handle.await;
    Ok(())
}
```

### ProviderCommand 枚举

```rust
#[derive(Debug)]
pub enum ProviderCommand {
    SwitchThink(bool),                // 启用/禁用思考模式
    ChangeModel {                     // 切换主模型
        character: String,
        agent: String,
        provider: String,
    },
    Send {                           // 发送消息给 Agent
        message: UserMessage,
    },
    Exit,                            // 安全退出
}
```

### ProviderResponse 枚举

```rust
#[derive(Debug)]
pub enum ProviderResponse {
    Response(BotResponse),           // 来自 Agent 的响应
    Error(String),                   // 执行过程中的错误
}
```

### Provider::run() 主循环

`Provider::run()` 是一个异步方法，接受两个通道参数：
- `cmd_rx: mpsc::Receiver<ProviderCommand>` - 命令接收通道
- `resp_tx: mpsc::Sender<ProviderResponse>` - 响应发送通道

**主循环功能**：
1. **Timer 系统** - 30秒轮询定时任务，触发时发送桌面通知
2. **AutoLoop 系统** - 每60秒 tick 一次，按配置间隔执行自动学习、反思、清理
3. **命令处理** - 处理来自外部的 ProviderCommand
4. **响应转发** - 将 BotResponse 流转发为 ProviderResponse

**LoopContinue 内部枚举**：
```rust
enum LoopContinue {
    Continue(bool),                              // true=继续循环, false=退出
    Response(mpsc::Receiver<BotResponse>),       // 携带新的响应接收通道
}
```

**循环内部结构**：
```rust
loop {
    tokio::select! {
        Some(cmd) = cmd_rx.recv() => {
            match self.handle_command(cmd).await {
                LoopContinue::Continue(true) => {}
                LoopContinue::Continue(false) => break,
                LoopContinue::Response(rx) => bot_response = rx,
            }
        }

        Some(notification) = self.timer_rx.recv() => {
            let _ = SystemNotify::send(&notification.character, &notification.body);
        }

        Some(content) = bot_response.recv() => {
            let _ = resp_tx.send(ProviderResponse::Response(content)).await;
        }

        _ = auto_loop_interval.tick() => {
            auto_loop_elapsed_minutes += 1;
            if let Some(al) = &mut self.auto_loop
                && al.tick(auto_loop_elapsed_minutes).await
                && let Some(core) = &mut self.core
            {
                let _ = al.run_once(core).await;
            }
        }

        _ = shutdown_rx_for_main.changed() => break,
    }
}
```

### handle_command 方法

`handle_command` 根据 `ProviderCommand` 变体派发：

| 命令 | 处理逻辑 |
|------|----------|
| `SwitchThink(enabled)` | 设置 `core.api_json.params.enable_thinking = enabled` |
| `ChangeModel { character, agent, provider }` | 调用 `core.config.agent.set_leader()` 切换模型 |
| `Send { message }` | 调用 `self.send(message)`，返回携带 BotResponse channel 的 `LoopContinue::Response` |
| `Exit` | 调用 `self.exit()`，返回 `LoopContinue::Continue(false)` 退出循环 |

### send 方法

根据 `UserMessage.mode` 路由：
- `SendMode::Task` → `core.task(message)`
- `SendMode::Chat` → `core.chat(message)`

---

## AutoLoop 模块

参见 `src/auto_loop/README.md`。

**核心功能**：
| 功能 | 描述 |
|------|------|
| **AutoStudy** | 自动学习模式，使用工具学习用户对话和项目 |
| **AutoReflect** | 自我反思，生成用户画像和经验总结 |
| **AutoClear** | 自动清理 note_book 和 skills_book 内容 |

**架构**：分为两个结构体
- `AutoLoopManager` (`mod.rs`) — 调度器，管理计时和并发锁，持有 `time_count`、`gap`、`loop_locked`（`AtomicBool`）
- `AutoLoop` (`auto.rs`) — 执行器，持有独立 `Core` 实例，实现 `auto_study`、`auto_reflect`、`auto_clear` 具体逻辑

`run_once(core)` 从 `AutoLoopManager` 发起，将 `Core` 引用传入后台 `tokio::spawn` 任务中执行，通过 `AtomicBool` 防止并发执行。

**配置参数**：
- `auto_loop_gap`: 执行间隔（分钟），默认 300 分钟
- 存储在 `~/.config/synapcore/synapcore.toml` 的 `[normal]` 部分
- `gap = 0` 时禁用 AutoLoop

**数据持久化**：
- **计时器缓存**: `~/.cache/synapcore_cache/cache.json`
- **反思文件**: `~/.config/synapcore/data/{character}_reflection.md`

---

## timer 模块

参见 `src/timer/README.md`。

**核心组件**：
| 组件 | 职责 |
|------|------|
| `Timer` | 定时任务数据结构 (id/time/character/prompt/done) |
| `TimerStore` | timer.json 读写层 |
| `TimerLoop` | 30s 轮询 + fire + 发送 TimerNotification |
| `TimerNotification` | fire 结果 (character + body)，传递给 Provider 主循环 |

**数据流**：
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

**错误类型**：`NotifyErr::Send(String)`

---

## 对其他 crate 的修改

### synapcore_core

1. **UserMessage 增强**：
   - 新增 `mode: SendMode` 和 `character: String` 字段
   - 新增 `UserMessage::task(text)` 和 `UserMessage::chat(character)` 工厂方法

2. **SendMode 枚举**：
   ```rust
   pub enum SendMode { Task, Chat }
   ```

3. **Core 新增方法**：
   - `Core::exit()` - 安全退出，保存配置和工具状态
   - `Core::task()` 添加任务提示词前缀
   - `Core::chat()` 聊天模式

4. **NormalConfig 新增字段**：
   ```rust
   pub auto_loop_gap: usize  // AutoLoop 执行间隔（分钟），0=禁用
   ```

5. **工具提示注入**：
   - 根据 `enable_tools` 自动注入系统提示
   - 为 leader 角色注入 skills_list

### synapcore_tools

1. **新增 timer Inner 工具**：
   - `add`: 添加定时任务 (time/character/prompt)
   - `list`: 列出未完成任务
   - `remove`: 按 ID 删除任务

2. **ToolResponse 新增变体**：
   ```rust
   Timer { action: String, content: String }
   ```

3. **Tools 新增方法**：
   - `Tools::get_skills_list()` - 获取技能列表
   - `Tools::exit()` - 安全退出，保存工具状态

---

## 配置文件更新

### synapcore.toml 新增字段

```toml
[normal]
# ... 原有字段 ...
auto_loop_gap = 300  # AutoLoop 执行间隔（分钟），默认 300
```

### 新增文件

| 文件 | 路径 | 用途 |
|------|------|------|
| timer.json | `~/.cache/synapcore_cache/timer.json` | 定时任务存储 |
| cache.json | `~/.cache/synapcore_cache/cache.json` | AutoLoop 计时缓存 |
| {character}_reflection.md | `~/.config/synapcore/data/` | 用户反思文档 |

---

## 核心工作流程

### 1. 启动流程

```
1. Provider::new()
   ↓
2. Core::init() + 创建 watch / mpsc 通道
   ↓
3. Provider::run(cmd_rx, resp_tx)
   ↓
4. Provider::timer_run() 启动 TimerLoop（异步任务，30s 轮询）
   ↓
5. Provider::auto_loop_run() 初始化 AutoLoopManager（从 config 读取 gap）
   ↓
6. 进入主循环 tokio::select!（5 路事件源）
```

### 2. 消息发送流程

```
1. 用户构造 UserMessage
   ↓
2. cmd_tx.send(ProviderCommand::Send { message })
   ↓
3. Provider::handle_command() 匹配 Send 分支
   ↓
4. Provider::send() → 根据 SendMode::Task/Chat 调用 core.task() / core.chat()
   ↓
5. BotResponse 流 → ProviderResponse::Response → resp_tx
   ↓
6. LoopContinue::Response 携带新的 BotResponse 接收通道返回主循环
```

### 3. 自动进化流程

```
每60秒 tick 一次（累计 auto_loop_elapsed_minutes）：
1. al.tick(elapsed_minutes) → time_count 累加
   ↓
2. time_count 是 gap 的倍数时返回 true，触发执行：
   ↓
3. al.run_once(core) （在 tokio::spawn 后台任务中执行）
   ↓
   ├── AutoStudy: 学习用户对话和项目
   ├── AutoReflect: 生成/更新反思文档
   └── AutoClear: 清理笔记和技能
```

---

## 错误处理

### AutoLoopErr（`pub(crate)`，仅模块内部可见）

```rust
pub(crate) enum AutoLoopErr {
    Io(std::io::Error),
    Serde(serde_json::Error),
    Core(CoreErr),
    Path(String),
    Regex(regex::Error),
}
pub(crate) type AutoLoopResult<T> = Result<T, AutoLoopErr>;
```

### TimerErr（公开）

```rust
pub enum TimerErr {
    Io(std::io::Error),
    Serde(serde_json::Error),
    TimeFormat(String),      // "YYYY-MM-DD-HH:mm" 格式校验失败
    PathNotFound(String),
    CoreInit(CoreErr),
    CoreChat(String),
    Regex(regex::Error),
}
```

### NotifyErr（公开）

```rust
pub enum NotifyErr {
    Send(String),
}
```

---

## 公共 API

`lib.rs` 中公开导出的类型：

| 导出项 | 来源模块 | 类型 |
|--------|----------|------|
| `ProviderCommand` | `provider_cmd` | Enum |
| `ProviderResponse` | `provider_cmd` | Enum |
| `SendMode` | `synapcore_core` | Enum (re-export) |
| `SystemNotify` | `notify` | Struct |
| `Timer` | `timer` | Struct |
| `TimerErr` | `timer` | Enum |
| `TimerNotification` | `timer` | Struct |
| `TimerStore` | `timer` | Struct |
| `Provider` | `provider` | Struct |

注意：`AutoLoopManager`、`AutoLoop`、`AutoLoopErr` 均为 `pub(crate)` 或 `pub(super)`，仅 crate 内部可见，对外不暴露。

---

## 设计理念

### 1. 通道驱动架构

Provider 采用完全异步的通道驱动设计：
- **命令通道**: 接收外部控制指令
- **响应通道**: 发送执行结果和Agent响应
- **内部通道**: Timer、AutoLoop等模块间通信

### 2. 模块化设计

每个功能模块独立：
- **timer**: 定时任务，与核心逻辑解耦
- **auto_loop**: 自动进化，独立计时和持久化
- **notify**: 系统通知，轻量级封装

### 3. 状态持久化

关键状态自动持久化：
- **定时任务**: timer.json
- **自动进化计时**: cache.json
- **用户反思**: {character}_reflection.md
- **工具状态**: 通过 Tools::exit() 保存

### 4. 错误隔离

各模块错误独立，避免级联失败：
- TimerLoop 失败不影响主循环
- AutoLoop 失败记录日志但继续运行
- 单个命令失败不影响其他命令处理

---

