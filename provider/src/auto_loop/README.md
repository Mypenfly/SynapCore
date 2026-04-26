# AutoLoop 模块

作为 Provider 的核心模块，实现 Agent 的自我进化功能，包括自动学习、自我反思和自动清理。

---

## 功能概述

| 功能 | 描述 |
|------|------|
| **AutoStudy** | 自动学习模式，使用工具学习用户对话和项目内容 |
| **AutoReflect** | 自我反思，生成用户画像和经验总结文档 |
| **AutoClear** | 自动清理 note_book 和 skills_book 内容 |

## 架构

分为两个结构体，分离调度与执行职责：

```
AutoLoopManager (mod.rs)          AutoLoop (auto.rs)
┌──────────────────────┐          ┌──────────────────────┐
│ time_count: usize    │ 管理计时  │                      │
│ gap: usize           │ ◄────────│ core: Core           │
│ loop_locked: AtomicBool│        │                      │
│                      │          │ auto_study()         │
│ run_once(core) ──────┼────────►│ auto_reflect()       │
│ tick()               │          │ auto_clear()         │
│ exit()               │          │                      │
└──────────────────────┘          └──────────────────────┘
```

- **AutoLoopManager**: 调度器，管理计时累计和并发锁
- **AutoLoop**: 执行器，持有独立 Core 实例，实现具体逻辑

## 配置参数

- **auto_loop_gap**: 执行间隔（分钟），默认 300 分钟。设为 `0` 表示禁用。
- 配置位置: `~/.config/synapcore/synapcore.toml` 的 `[normal]` 部分

```toml
[normal]
auto_loop_gap = 300  # AutoLoop 执行间隔（分钟）
```

---

## 数据结构

### AutoLoopCache
计时器缓存，存储在 `~/.cache/synapcore_cache/cache.json`

```rust
pub struct AutoLoopCache {
    pub time_count: usize,      // 累计计时（分钟）
    pub last_run: Option<u64>,  // 上次执行时间戳
}
```

### AutoLoopManager
调度器结构体，`pub(crate)` 可见

```rust
pub(crate) struct AutoLoopManager {
    time_count: usize,                     // 当前累计计时（分钟）
    gap: usize,                            // 执行间隔
    loop_locked: Arc<AtomicBool>,          // 防止并发执行锁
}
```

### AutoLoop
执行器结构体，`pub(super)` 可见

```rust
pub(super) struct AutoLoop {
    core: Core,
}
```

---

## 核心方法

### AutoLoopManager 方法

| 方法 | 说明 |
|------|------|
| `new(gap) -> AutoLoopResult<Self>` | 从缓存加载计时，初始化并发锁为 false |
| `tick(elapsed_minutes) -> bool` | 累计时间，当 `time_count` 是 `gap` 的倍数时返回 true |
| `run_once(core) -> AutoLoopResult<()>` | 在 `tokio::spawn` 后台任务中执行 auto_study → auto_reflect → auto_clear |
| `exit() -> AutoLoopResult<()>` | 保存当前 time_count 到 cache.json |

**tick 逻辑**：
```rust
pub async fn tick(&mut self, elapsed_minutes: usize) -> bool {
    if self.gap == 0 {
        return false;  // gap=0 时禁用
    }
    self.time_count += elapsed_minutes;
    if self.time_count >= self.gap {
        self.time_count -= self.gap;  // 减掉 gap，保留剩余
        return true;
    }
    false
}
```

**run_once 并发控制**：
```rust
pub async fn run_once(&mut self, core: &mut Core) -> AutoLoopResult<()> {
    if self.loop_locked.load(Ordering::SeqCst) {
        return Ok(());  // 已有循环在执行
    }
    self.loop_locked.store(true, Ordering::SeqCst);

    let locked = self.loop_locked.clone();

    tokio::spawn(async move {
        let mut auto = AutoLoop::new(core).unwrap();
        let _ = auto.auto_study().await;
        let _ = auto.auto_reflect().await;
        let _ = auto.auto_clear().await;
        locked.store(false, Ordering::SeqCst);
    });

    Ok(())
}
```

### AutoLoop 方法

| 方法 | 说明 |
|------|------|
| `new(core) -> AutoLoopResult<Self>` | 包装一个 Core 实例 |
| `auto_study() -> AutoLoopResult<()>` | 自动学习模式 |
| `auto_reflect() -> AutoLoopResult<()>` | 自我反思模式 |
| `auto_clear() -> AutoLoopResult<()>` | 自动清理模式 |

---

## 详细实现

### AutoStudy - 自动学习

**提示词**：
```
[System command]现在是AutoStudy模式，
请你详细使用各式工具进行学习，内容包括但不限于最近和用户进行的交流，
最近在做的项目，学习内容要使用skills_book工具规范记录，
学习过程建议使用files_extract(学习现有项目)，web_search(查找有关资料)。
特别注意此次任务对话记录和工具调用记录不会保存，你写在skills_book,和note_book中的内容就是你以后参照的标准
```

**执行流程**：
1. 使用传入的 Core 实例（leader 角色）
2. `enable_tools = true`，允许使用所有工具
3. `is_save = false`，不保存此次对话记录
4. 自动注入工具调用权限提示

**设计理念**：
- Agent 通过工具自主探索和学习
- 学习成果通过 skills_book 和 note_book 持久化
- 避免污染用户的正式对话记录

### AutoReflect - 自我反思

**提示词格式**：
包含详细的反思模板，要求输出 `<reflection>...</reflection>` 标签内容。

**反思文档结构**：
```
<reflection>

## 用户画像 (User Profile)

- **基本信息**: [性别, 年龄, 职业等]
- **兴趣领域**: [技术, 学习, 工作, 生活等]
- **沟通风格**: [直接, 委婉, 技术型, 实用型等]
- **知识水平**: [初级, 中级, 高级, 专家等]

## 对话模式观察 (Conversation Patterns)

- **常见问题类型**: [技术问题, 学习求助, 工作咨询, 生活建议等]
- **回应偏好**: [详细解释, 简短回答, 代码示例, 理论说明等]

## 经验总结 (Experience Summary)

1. **有效策略**: [哪些方法对该用户特别有效]
2. **无效策略**: [哪些方法效果不佳或应避免]
3. **成功案例**: [特别成功的交互案例]
4. **改进建议**: [未来交互中可以改进的地方]

## 知识积累 (Knowledge Accumulation)

- **已掌握技能**: [用户已经学会的技能或知识]
- **正在学习**: [用户当前正在学习的内容]
- **知识缺口**: [用户可能需要的但尚未掌握的知识]

## 关系质量评估 (Relationship Quality)

- **信任程度**: [低, 中, 高]
- **合作顺畅度**: [顺畅, 一般, 需要改进]
- **沟通效率**: [高效, 正常, 有待提高]

## 角色构建与自我认知 (Role-Building & Self-Awareness)

- [Agent 对自己的角色定位和认知]
- [在不同场景下的角色适应策略]

## 注意事项 (Notes)

1. 保持客观, 基于实际交互数据
2. 避免主观臆断
3. 定期更新, 反映最新状态
4. 格式保持简洁明了

## 时间戳

- **上次更新**: [YYYY-MM-DD HH:MM:SS]

</reflection>
```

**执行流程**：
1. 读取现有反思文档：`~/.config/synapcore/data/{character}_reflection.md`
2. 构造包含现有内容的提示词
3. 调用 Core，要求输出 `<reflection>` 标签格式
4. 通过正则提取标签内容，覆盖写入反思文档

**关键参数**：
```rust
const REFLECTION_TAG: &str = "reflection";
```

**文件位置**：
- `~/.config/synapcore/data/Yore_reflection.md` (示例)

### AutoClear - 自动清理

**提示词**：
```
[System Command]现在是AutoClear模式，请对note_book和skills_book的内容进行清理，
建议对已经失去效力或者长期不用的note和skill进行清理（建议清理启动数量 >= 20）。
请开始清理工作。
```

**执行流程**：
1. `enable_tools = true`，允许使用 note_book 和 skills_book 工具
2. `is_save = false`，不保存清理对话记录
3. Agent 自主决定清理策略

**设计理念**：
- Agent 自主管理自己的记忆和技能
- 避免信息过载，保持系统高效
- 清理标准由 Agent 根据上下文判断

---

## 错误处理

### AutoLoopErr 枚举（`pub(crate)`）

```rust
pub(crate) enum AutoLoopErr {
    Io(std::io::Error),           // 文件读写错误
    Serde(serde_json::Error),     // JSON 序列化错误
    Core(CoreErr),                // Core 调用错误
    Path(String),                 // 路径获取错误
    Regex(regex::Error),          // 正则表达式错误
}
pub(crate) type AutoLoopResult<T> = Result<T, AutoLoopErr>;
```

### 错误隔离策略

1. **模块独立**：auto_study / auto_reflect / auto_clear 各自独立 try 处理，失败不中断后续
2. **日志记录**：错误通过 `eprintln!` 输出到 stderr
3. **降级运行**：单个功能失败不影响其他功能执行

---

## 集成到 Provider

### 初始化集成

```rust
// 在 Provider::run() 开始时初始化
self.auto_loop_run()?;

// 内部实现
fn auto_loop_run(&mut self) -> CoreResult<()> {
    let gap = self.core.config.normal.auto_loop_gap;
    if gap > 0 {
        self.auto_loop = Some(AutoLoopManager::new(gap)?);
    }
    Ok(())
}
```

### 主循环集成

```rust
let mut auto_loop_interval = tokio::time::interval(Duration::from_secs(60));
let mut auto_loop_elapsed_minutes = 0;

// 在主循环的 tokio::select! 中
_ = auto_loop_interval.tick() => {
    auto_loop_elapsed_minutes += 1;
    if let Some(al) = &mut self.auto_loop
        && al.tick(auto_loop_elapsed_minutes).await
        && let Some(core) = &mut self.core
    {
        if let Err(e) = al.run_once(core).await {
            eprintln!("[Provider] auto loop error : {}", e);
        }
    }
}
```

### 安全退出集成

```rust
if let Some(al) = &self.auto_loop {
    if let Err(e) = al.exit() {
        eprintln!("[Provider] auto loop error : {}", e);
    }
}
```

---

## 文件系统

### 缓存文件

```
~/.cache/synapcore_cache/cache.json
```

**内容**：
```json
{
  "time_count": 150,
  "last_run": 1775820012
}
```

### 反思文档

```
~/.config/synapcore/data/{character}_reflection.md
```

**示例**：
```markdown
<reflection>
## 用户画像 (User Profile)

- **基本信息**: [男性, 25-30岁, 软件工程师]
- **兴趣领域**: [编程技术, Rust语言, AI助手开发, 系统架构]
- **沟通风格**: [技术型, 直接, 注重细节]
- **知识水平**: [高级, 有丰富的编程和系统设计经验]

## 对话模式观察 (Conversation Patterns)

- **常见问题类型**: [技术实现, 架构设计, 代码审查, 学习指导]
- **回应偏好**: [详细的技术解释, 代码示例, 架构图说明]

## 经验总结 (Experience Summary)

1. **有效策略**: [提供具体的代码示例, 使用架构图说明, 分步骤解释]
2. **无效策略**: [过于抽象的描述, 没有实际示例的理论]
3. **成功案例**: [帮助设计 SynapCore 系统架构, 提供详细的实现方案]
4. **改进建议**: [更多使用图表说明复杂架构]

## 知识积累 (Knowledge Accumulation)

- **已掌握技能**: [Rust编程, Tokio异步, LLM API集成, 向量数据库]
- **正在学习**: [更复杂的AI Agent架构, 多模态处理]
- **知识缺口**: [大规模系统部署, 性能优化高级技巧]

## 关系质量评估 (Relationship Quality)

- **信任程度**: [高]
- **合作顺畅度**: [顺畅]
- **沟通效率**: [高效]

## 角色构建与自我认知 (Role-Building & Self-Awareness)

- [Agent 对自己的角色定位和认知]
- [在不同场景下的角色适应策略]

## 注意事项 (Notes)

1. 保持客观, 基于实际交互数据
2. 避免主观臆断
3. 定期更新, 反映最新状态
4. 格式保持简洁明了

## 时间戳

- **上次更新**: [2026-04-25 22:52:19]

</reflection>
```

---

## 设计理念

### 1. 调度与执行分离

- `AutoLoopManager` 负责"何时运行"，管理计时、持久化、并发锁
- `AutoLoop` 负责"运行什么"，包含具体的 AI 交互逻辑
- `run_once(core)` 将 Core 引用传给后台 `tokio::spawn` 任务

### 2. 并发安全

- 通过 `AtomicBool` 锁确保同一时间只有一个 AutoLoop 任务在执行
- 即使 tick 多次触发，也不会叠加执行

### 3. 持久化计时

- 解决应用重启导致计时重置的问题
- 累计计时，确保按间隔执行

### 4. 自我管理

- Agent 自主决定学习内容和清理策略
- 反思文档由 Agent 生成和维护，包含角色构建与自我认知

### 5. 错误容忍

- 单个功能失败不影响整体（每个步骤独立 try）
- 后台任务失败通过锁释放保证下次可重试

---

## 使用示例

### 独立使用 AutoLoop（仅内部可见，不可直接外部引用）

AutoLoop 和 AutoLoopManager 均为 `pub(crate)` / `pub(super)`，不对外暴露。以下为内部使用示意：

```rust
// auto_loop/mod.rs - AutoLoopManager::run_once()
let mut auto = AutoLoop::new(core)?;
auto.auto_study().await?;
auto.auto_reflect().await?;
auto.auto_clear().await?;
```

### 在 Provider 中使用

```rust
// Provider 主循环中自动集成
// 无需额外配置，只需在 synapcore.toml 中设置 auto_loop_gap

// 查看反思文档
let reflection_content = std::fs::read_to_string(
    dirs::config_dir()
        .unwrap()
        .join("synapcore/data/Yore_reflection.md")
)?;

println!("当前反思文档：\n{}", reflection_content);
```

---

## 配置建议

### 开发环境
```toml
auto_loop_gap = 30  # 30分钟，便于测试
```

### 生产环境
```toml
auto_loop_gap = 300  # 5小时，避免频繁打扰
```

### 禁用 AutoLoop
```toml
auto_loop_gap = 0  # tick() 直接返回 false，永不执行
```

---

## 监控和调试

### 日志输出
```
[AutoLoop] 开始自动学习...
[AutoLoop] 开始自我反思...
[AutoLoop] 开始清理工作...
[AutoLoop] 完成一轮自动循环
```

### 检查文件
```bash
# 查看缓存状态
cat ~/.cache/synapcore_cache/cache.json

# 查看反思文档
cat ~/.config/synapcore/data/Yore_reflection.md

# 查看执行日志
journalctl -u synapcore  # 如果配置了系统服务
```

---

**基于代码版本**: AutoLoopManager + AutoLoop 分离架构（mod.rs + auto.rs）