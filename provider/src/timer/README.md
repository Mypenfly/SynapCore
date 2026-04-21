# Timer 模块 — 定时任务系统

> 提供定时任务的数据定义、持久化存储、轮询调度与执行能力

---

## 架构

```
timer/mod.rs
├── Timer               # 单条定时任务数据结构
├── TimerStore          # timer.json 读写层
├── TimerLoop           # 异步轮询循环（调度 + 执行）
├── TimerErr            # 错误枚举
└── 辅助函数
    ├── default_timer_path()   # 默认 timer.json 路径
    ├── extract_tag_content()  # XML 标签内容提取
    └── truncate_str()         # 字符截断
```

---

## 数据结构

### Timer

```rust
pub struct Timer {
    pub id: String,        // UUID v4，唯一标识
    pub time: String,      // 目标时间，格式: "YYYY-MM-DD-HH:mm"
    pub character: String, // 执行角色名（对应 core 配置中的角色）
    pub prompt: String,    // 触发时发给该角色的提示词
    pub done: bool,        // 是否已执行
}
```

- **`Timer::new(time, character, prompt)`**: 创建前自动校验时间格式，ID 自动生成
- **`Timer::is_due()`**: 将 `time` 与 `Local::now()` 比较，判断是否到期

**时间格式要求**: `YYYY-MM-DD-HH:mm`，如 `2026-04-21-08:23`，使用 chrono 的 `NaiveDateTime` 解析验证

### timer.json 示例

**存储路径**: `~/.cache/synapcore_cache/timer.json`

```json
[
  {
    "id": "a1b2c3d4-...",
    "time": "2026-04-22-09:00",
    "character": "Yore",
    "prompt": "提醒我开会",
    "done": false
  },
  {
    "id": "e5f6g7h8-...",
    "time": "2026-04-21-18:30",
    "character": "Yore",
    "prompt": "该下班了",
    "done": true
  }
]
```

---

## TimerStore

| 方法 | 说明 |
|------|------|
| `load(path)` | 加载 timer.json；文件不存在则创建空数组 |
| `reload()` | 重新从磁盘读取（供轮询时检测外部变更） |
| `add(timer)` | 添加任务并立即持久化 |
| `mark_done(id)` | 标记为已完成并持久化 |
| `remove(id)` | 按 ID 删除任务并持久化 |
| `pending()` | 返回所有 `done=false` 的任务引用 |

每次写操作（add/mark_done/remove）都会立即 `save()` 写回磁盘，保证持久性。

---

## TimerLoop

```
┌─────────────────────────────────────────────────┐
│                  TimerLoop                       │
│  ┌──────────┐  ┌──────────┐  ┌───────────────┐  │
│  │TimerStore │  │   Core   │  │shutdown_rx    │  │
│  │(timer.json)│  │(独立实例) │  │(watch通道)    │  │
│  └──────────┘  └──────────┘  └───────────────┘  │
└──────────────────────┬──────────────────────────┘
                       │
                run() 异步循环
                       │
            ┌──────────┴──────────┐
            │   tokio::select!    │
            ├─────────┬───────────┤
            │ 30s轮询  │ shutdown  │
            │ tick     │ 信号      │
            ▼          │           │
     check_and_fire()  │           │
        ┌─────┐       │           │
        │reload│       │           │
        │  ↓  │       │           │
        │筛选  │       │           │
        │is_due│       │           │
        │  ↓  │       │           │
        │fire()│       │           │
        │  ↓  │       │           │
        │mark  │       │           │
        │_done │       │           │
        └─────┘       │           │
                       ▼          ▼
                    退出循环
```

### 关键设计

- **独立 Core 实例**: `TimerLoop::new()` 调用 `Core::init()` 创建专属实例，与主交互 Core 互不干扰
- **轮询间隔**: 30 秒（`POLL_INTERVAL_SECS`）
- **退出机制**: 通过 `watch::Receiver<bool>` 接收 shutdown 信号，收到后立即退出循环

### fire() 执行流程

1. 在 `timer.prompt` 前注入系统指令，要求 agent 用 `<timer>内容</timer>` 格式输出，且不超过 50 字
2. 构造 `UserMessage`（`mode=Chat`, `enable_tools=false`, `is_save=false`）
3. 调用 `core.chat(&timer.character, &message)` 发起对话
4. 收集完整 `BotResponse::Content` 流
5. 用 `extract_tag_content()` 提取 `<timer>` 标签内容
6. 若提取失败，回退到截取完整输出
7. `truncate_str()` 确保不超过 50 字（按字符计数，超出加 `...`）

### 使用示例

```rust
use synapcore_provider::timer::{TimerLoop, Timer, TimerStore};
use tokio::sync::watch;

#[tokio::main]
async fn main() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // 手动添加一条定时任务
    let mut store = TimerStore::load(&default_timer_path()).unwrap();
    let timer = Timer::new(
        "2026-04-22-09:00".to_string(),
        "Yore".to_string(),
        "提醒我开会".to_string(),
    ).unwrap();
    store.add(timer).unwrap();

    // 启动循环
    let mut loop_ = TimerLoop::new(shutdown_rx).unwrap();
    loop_.run().await.unwrap();

    // 需要退出时
    let _ = shutdown_tx.send(true);
}
```

---

## TimerErr

| 变体 | 来源 |
|------|------|
| `Io(io::Error)` | 文件读写失败 |
| `Serde(serde_json::Error)` | JSON 序列化/反序列化失败 |
| `TimeFormat(String)` | 时间字符串不符合 `YYYY-MM-DD-HH:mm` |
| `PathNotFound(String)` | 路径不存在 |
| `CoreInit(CoreErr)` | Core::init() 失败 |
| `CoreChat(String)` | Core 对话过程中出错 |
| `Regex(regex::Error)` | 正则表达式编译失败 |

---

## 常量

| 常量 | 值 | 说明 |
|------|----|------|
| `TIMER_TAG` | `"timer"` | agent 输出中的 XML 标签名 |
| `TIMER_BODY_MAX_LEN` | `50` | 提取内容的最大字符数 |
| `POLL_INTERVAL_SECS` | `30` | 轮询间隔（秒） |
