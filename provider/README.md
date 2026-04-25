# SynapCore Provider

> **crate名**: `synapcore_provider`  
> **定位**: Core 的扩展层，提供定时任务、统一消息发送、系统通知、自动进化循环能力  
> **依赖**: `synapcore_core`, `tools`, `tokio`, `notify-rust`, `serde`, `serde_json`, `chrono`, `regex`, `uuid`, `dirs`, `thiserror`

---

## 架构

```
provider/src
├── lib.rs                  # Provider 主入口
├── provider_cmd.rs         # ProviderCommand / ProviderResponse 定义
├── timer/
│   ├── mod.rs              # Timer / TimerStore / TimerLoop / TimerNotification
│   └── README.md           # timer 模块详细文档
├── auto_loop/
│   ├── mod.rs              # AutoLoop (AutoStudy / AutoReflect / AutoClear)
│   └── README.md           # auto_loop 模块详细文档
└── notify/
    └── mod.rs              # SystemNotify (notify-rust 封装)
```

---

## Provider 主入口

### 初始化与基本使用

```rust
use synapcore_provider::{Provider, ProviderCommand, ProviderResponse};
use synapcore_core::UserMessage;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 Provider 实例
    let provider = Provider::new()?;
    
    // 创建命令/响应通道
    let (cmd_tx, cmd_rx) = mpsc::channel::<ProviderCommand>(1024);
    let (resp_tx, mut resp_rx) = mpsc::channel::<ProviderResponse>(1024);
    
    // 启动 Provider 主循环（异步任务）
    let provider_handle = tokio::spawn(async move {
        if let Err(e) = provider.run(cmd_rx, resp_tx).await {
            eprintln!("Provider 运行失败: {}", e);
        }
    });
    
    // 发送命令
    let message = UserMessage::task("你好");
    cmd_tx.send(ProviderCommand::Send { message }).await?;
    
    // 接收响应
    while let Some(resp) = resp_rx.recv().await {
        match resp {
            ProviderResponse::Response(bot_resp) => {
                println!("{}", bot_resp);
            }
            ProviderResponse::Error(err) => {
                eprintln!("错误: {}", err);
            }
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
2. **AutoLoop 系统** - 按配置间隔执行自动学习、反思、清理
3. **命令处理** - 处理来自外部的 ProviderCommand
4. **响应转发** - 将 BotResponse 转发为 ProviderResponse

**循环内部结构**：
```rust
loop {
    tokio::select! {
        // 1. 处理命令
        Some(cmd) = cmd_rx.recv() => { self.handle_command(cmd).await }
        
        // 2. 处理 Timer 通知
        Some(notification) = self.timer_rx.recv() => { SystemNotify::send(...) }
        
        // 3. 转发 BotResponse
        Some(content) = bot_response.recv() => { resp_tx.send(ProviderResponse::Response(content)) }
        
        // 4. AutoLoop 计时
        _ = auto_loop_interval.tick() => { self.auto_loop.tick(...) }
        
        // 5. 检查 shutdown 信号
        _ = shutdown_rx.changed() => { break }
    }
}
```

---

## AutoLoop 模块

参见 `src/auto_loop/README.md`。

**核心功能**：
| 功能 | 描述 |
|------|------|
| **AutoStudy** | 自动学习模式，使用工具学习用户对话和项目 |
| **AutoReflect** | 自我反思，生成用户画像和经验总结 |
| **AutoClear** | 自动清理 note_book 和 skills_book 内容 |

**配置参数**：
- `auto_loop_gap`: 执行间隔（分钟），默认 300 分钟
- 存储在 `~/.config/synapcore/synapcore.toml` 的 `[normal]` 部分

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

4. **NormalConfig 新增字段**：
   ```rust
   pub auto_loop_gap: usize  // AutoLoop 执行间隔（分钟）
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
2. Core::init() + 初始化通道
   ↓
3. Provider::run(cmd_rx, resp_tx)
   ↓
4. 启动 TimerLoop (异步任务)
   ↓
5. 初始化 AutoLoop
   ↓
6. 进入主循环
```

### 2. 消息发送流程

```
1. 用户构造 UserMessage
   ↓
2. cmd_tx.send(ProviderCommand::Send { message })
   ↓
3. Provider::handle_command() 匹配 Send 分支
   ↓
4. Provider::send() → Core::task()/chat()
   ↓
5. BotResponse 流 → ProviderResponse::Response
   ↓
6. resp_tx 发送给调用方
```

### 3. 自动进化流程

```
每分钟检查：
1. auto_loop.tick(elapsed_minutes)
   ↓
2. 达到 auto_loop_gap 时执行：
   ↓
3. auto_loop.run_once()
   ↓
   ├── AutoStudy: 学习用户对话和项目
   ├── AutoReflect: 生成/更新反思文档
   └── AutoClear: 清理笔记和技能
```

---

## 错误处理

### AutoLoopErr

```rust
pub enum AutoLoopErr {
    Io(std::io::Error),
    Serde(serde_json::Error),
    Core(CoreErr),
    Path(String),
    Regex(regex::Error),
}
```

### TimerErr

```rust
pub enum TimerErr {
    Io(std::io::Error),
    Serde(serde_json::Error),
    TimeFormat(String),
    PathNotFound(String),
    CoreInit(CoreErr),
    CoreChat(String),
    Regex(regex::Error),
}
```

### NotifyErr

```rust
pub enum NotifyErr {
    Send(String),
}
```

---

## 使用示例

### 完整示例

```rust
use synapcore_provider::{Provider, ProviderCommand, ProviderResponse};
use synapcore_core::{UserMessage, SendMode};
use tokio::sync::mpsc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化 Provider
    let provider = Provider::new()?;
    
    // 2. 创建通道
    let (cmd_tx, cmd_rx) = mpsc::channel::<ProviderCommand>(1024);
    let (resp_tx, mut resp_rx) = mpsc::channel::<ProviderResponse>(1024);
    
    // 3. 启动 Provider（异步任务）
    let handle = tokio::spawn(async move {
        provider.run(cmd_rx, resp_tx).await.unwrap();
    });
    
    // 4. 发送各种命令
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // 切换思考模式
    cmd_tx.send(ProviderCommand::SwitchThink(true)).await?;
    
    // 切换模型
    cmd_tx.send(ProviderCommand::ChangeModel {
        character: "Yore".to_string(),
        agent: "deepseek".to_string(),
        provider: "siliconflow".to_string(),
    }).await?;
    
    // 发送消息
    let mut message = UserMessage::task("帮我分析这段代码");
    message.files = vec!["./src/main.rs".to_string()];
    cmd_tx.send(ProviderCommand::Send { message }).await?;
    
    // 5. 接收响应
    let mut response_count = 0;
    while let Some(resp) = resp_rx.recv().await {
        match resp {
            ProviderResponse::Response(bot_resp) => {
                println!("{}", bot_resp);
                response_count += 1;
                if response_count >= 5 {
                    break;
                }
            }
            ProviderResponse::Error(err) => {
                eprintln!("错误: {}", err);
            }
        }
    }
    
    // 6. 安全退出
    cmd_tx.send(ProviderCommand::Exit).await?;
    
    // 7. 等待 Provider 结束
    let _ = handle.await;
    
    println!("Provider 已安全退出");
    Ok(())
}
```

### 仅使用发送功能

```rust
use synapcore_provider::{Provider, ProviderCommand, ProviderResponse};
use synapcore_core::UserMessage;
use tokio::sync::mpsc;

async fn send_simple_message() -> Result<(), Box<dyn std::error::Error>> {
    let provider = Provider::new()?;
    let (cmd_tx, cmd_rx) = mpsc::channel::<ProviderCommand>(1024);
    let (resp_tx, mut resp_rx) = mpsc::channel::<ProviderResponse>(1024);
    
    tokio::spawn(async move {
        provider.run(cmd_rx, resp_tx).await.unwrap();
    });
    
    let message = UserMessage::task("你好，请介绍一下自己");
    cmd_tx.send(ProviderCommand::Send { message }).await?;
    
    while let Some(resp) = resp_rx.recv().await {
        if let ProviderResponse::Response(bot_resp) = resp {
            println!("{}", bot_resp);
        }
    }
    
    Ok(())
}
```

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

