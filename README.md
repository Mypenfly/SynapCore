# SynapCore - 多智能体AI编排系统

## 概述

SynapCore是一个基于Rust的多智能体AI编排系统，采用模块化设计，包含核心库、工具系统、Provider扩展层和TUI界面。项目采用异步通道驱动架构，支持LLM对话管理、记忆系统、工具调用、定时任务和自动进化功能。

(Mypenfly的个人项目，现阶段只供个人练习，主要功能尚未完善)

## 目录

1. [项目架构](#项目架构)
2. [核心模块](#核心模块)
3. [技术栈](#技术栈)
4. [快速开始](#快速开始)
5. [配置文件](#配置文件)
6. [开发指南](#开发指南)
7. [贡献指南](#贡献指南)

## 正文

### 1. 项目架构

#### 概述
SynapCore采用分层架构设计，各模块职责清晰，通过异步通道进行通信。

#### 引用文件
- `/home/mypenfly/projects/synapcore/Cargo.toml` - 工作空间配置
- `/home/mypenfly/projects/synapcore/flake.nix` - Nix开发环境配置

#### 正文
项目采用workspace组织，包含以下主要模块：

```
synapcore/
├── core/          # 核心库 - LLM对话管理、记忆系统、工具调用
├── tools/         # 工具系统 - 为LLM提供可调用的内部/外部工具
├── provider/      # Provider扩展层 - 定时任务、自动进化、统一消息发送
├── tui/           # TUI界面 - 基于ratatui的终端用户界面
└── src-flutter/   # Flutter前端 - 图形用户界面（待开发）
```

**架构特点**：
- **异步通道驱动**：使用tokio的mpsc通道实现模块间通信
- **模块化设计**：各模块独立，便于扩展和维护
- **配置驱动**：所有功能通过配置文件动态调整
- **记忆系统**：基于向量数据库的长期记忆存储和检索

#### 总结/建议
- 建议保持模块间的松耦合设计
- 异步通道是核心通信机制，需确保正确使用
- 配置系统应支持热重载

### 2. 核心模块

#### 概述
详细介绍各模块的功能和职责。

#### 引用文件
- `/home/mypenfly/projects/synapcore/core/README.md` - Core模块详细文档
- `/home/mypenfly/projects/synapcore/tools/README.md` - Tools模块详细文档
- `/home/mypenfly/projects/synapcore/provider/README.md` - Provider模块详细文档

#### 正文

**2.1 Core模块 (synapcore_core)**
- **定位**：系统核心库，提供LLM对话管理、记忆系统、工具调用、会话持久化
- **关键组件**：
  - `Core`：对外主入口，提供`task()`和`chat()`方法
  - `Assistant`：LLM会话实例管理
  - `MemoryStore`：向量记忆系统（SQLite + sqlite-vec）
  - `LLMClient`：HTTP请求与流式解析
  - `Session`：会话管理
- **特性**：
  - 事件驱动状态机（`CoreEvent`枚举）
  - 支持工具调用循环
  - 自动记忆存储（对话轮数达到阈值时触发）
  - 多模态支持（文本、图片、文件）

**2.2 Tools模块 (tools)**
- **定位**：为LLM agent提供可调用的内部/外部工具集
- **工具分类**：
  - **Inner工具**：内置工具（files_extract, web_search, files_write等）
  - **Outer工具**：通过shell执行第三方CLI工具
- **配置**：`~/.config/synapcore/tools/tools.toml`
- **特性**：
  - 动态工具搜索和添加
  - 统一的Tool trait接口
  - 支持参数验证和错误处理

**2.3 Provider模块 (synapcore_provider)**
- **定位**：Core的扩展层，提供高级功能
- **关键功能**：
  - **定时任务系统**：TimerLoop 30秒轮询，触发桌面通知
  - **自动进化系统**：AutoLoop（AutoStudy、AutoReflect、AutoClear）
  - **统一消息发送**：根据SendMode自动路由到task()或chat()
  - **系统通知**：notify-rust封装
- **架构**：完全异步的通道驱动设计

**2.4 TUI模块 (tui)**
- **定位**：基于ratatui的终端用户界面
- **状态**：基础结构已搭建，待与Provider深度集成

#### 总结/建议
- Core模块是系统基础，需保持稳定
- Tools模块应易于扩展，支持自定义工具
- Provider模块的AutoLoop是实现自我进化的关键
- TUI界面需要与Provider的命令/响应系统集成

### 3. 技术栈

#### 概述
项目采用现代化的Rust技术栈，支持多语言开发。

#### 引用文件
- `/home/mypenfly/projects/synapcore/flake.nix` - 开发环境依赖

#### 正文

**核心技术栈**：
- **Rust**：核心系统，edition 2024
  - tokio：异步运行时
  - reqwest：HTTP客户端
  - serde：序列化/反序列化
  - sqlite-vec：向量数据库扩展
  - ratatui：TUI框架

- **Python**：Agent开发
  - uv：Python包管理

- **Dart/Flutter**：前端界面
  - 跨平台移动端和桌面端

- **Nushell**：运维脚本
  - 统一的命令行工作流

**开发工具链**：
- Cargo workspace管理
- Nix提供可重现的开发环境
- cargo-watch热重载开发
- cargo-audit安全检查

#### 总结/建议
- Rust的异步生态成熟，适合构建高性能AI系统
- 多语言栈增加了复杂性，但提供了灵活性
- Nix确保开发环境一致性

### 4. 快速开始

#### 概述
快速上手SynapCore的步骤。

#### 引用文件
- 无特定文件，基于项目配置

#### 正文

**4.1 环境准备**
```bash
# 使用Nix开发环境
nix develop

# 或手动安装依赖
cargo install
```

**4.2 配置初始化**
```bash
# Core会自动创建默认配置
# 配置文件位置：~/.config/synapcore/
```

**4.3 基本使用**

**使用Core直接交互**：
```rust
use synapcore_core::{Core, UserMessage};

let mut core = Core::init()?;
let message = UserMessage::task("你好");
let mut rx = core.task(&message).await?;

while let Some(response) = rx.recv().await {
    print!("{}", response);
}
```

**使用Provider**：
```rust
use synapcore_provider::{Provider, ProviderCommand, ProviderResponse};

let provider = Provider::new()?;
let (cmd_tx, cmd_rx) = mpsc::channel(1024);
let (resp_tx, mut resp_rx) = mpsc::channel(1024);

// 启动Provider
tokio::spawn(async move {
    provider.run(cmd_rx, resp_tx).await;
});

// 发送消息
let message = UserMessage::task("分析代码");
cmd_tx.send(ProviderCommand::Send { message }).await?;

// 接收响应
while let Some(resp) = resp_rx.recv().await {
    match resp {
        ProviderResponse::Response(bot_resp) => println!("{}", bot_resp),
        ProviderResponse::Error(err) => eprintln!("错误: {}", err),
    }
}
```

#### 总结/建议
- 初次使用建议从Core模块开始
- Provider提供了更完整的生命周期管理
- 配置系统需要正确设置API密钥

### 5. 配置文件

#### 概述
SynapCore采用多层配置文件系统。

#### 引用文件
- 各模块的README中提到的配置文件结构

#### 正文

**5.1 配置文件结构**
```
~/.config/synapcore/
├── synapcore.toml          # 主配置
├── api.json               # API密钥和模型配置
├── prompts/               # 提示词模板
│   ├── {character}.md    # 角色提示词
│   └── memory.md         # 记忆提示词
├── tools/                # 工具配置
│   └── tools.toml        # 工具启用/禁用
└── data/                 # 数据文件
    └── {character}_reflection.md  # 反思文档
```

**5.2 关键配置说明**

**synapcore.toml**：
```toml
[normal]
sc_root = "~/.config/synapcore"
api_path = "~/.config/synapcore/api.json"
store_num = 50                     # 触发记忆存储的对话轮数
auto_loop_gap = 300                # AutoLoop执行间隔（分钟）

[agent]
[agent.leader]
character = "Yore"                 # 主角色名
agent = "deepseek"                 # 模型名
provider = "siliconflow"           # 供应商名

[[agent.subagents]]
character = "coder"                # 子角色名
agent = "gpt4o"
provider = "openai"

[memory]
min_score = 0.05                   # 记忆最低分数
max_score = 9.0                    # 高分注入阈值
top_k = 3                          # 检索返回的记忆数量
```

**api.json**：
```json
{
  "providers": [
    {
      "name": "siliconflow",
      "base_url": "https://api.siliconflow.cn/v1",
      "api_key": "YOUR_API_KEY",
      "models": [
        { "name": "deepseek", "model_id": "deepseek-ai/DeepSeek-V3" },
        { "name": "qwen_embed", "model_id": "BAAI/bge-large-zh-v1.5" }
      ]
    }
  ]
}
```

**5.3 工具配置 (tools.toml)**
```toml
sandbox_path = "/path/to/sandbox"
sandbox_dyn = true

[[inner]]
name = "files_extract"
enable = true

[[inner]]
name = "web_search"
enable = true

[inner.params]
base_url = "https://..."
api_key = "YOUR_KEY"
```

#### 总结/建议
- 配置文件采用TOML和JSON格式，易于阅读和编辑
- 建议使用环境变量管理敏感信息（如API密钥）
- 配置应支持热重载，无需重启服务

### 6. 开发指南

#### 概述
为开发者提供的扩展和贡献指南。

#### 引用文件
- `/home/mypenfly/projects/synapcore/tools/README.md` - 工具开发规范

#### 正文

**6.1 添加新工具**

参考Tools模块的README，创建Inner工具的步骤：

1. 在`tools/src/`下创建模块目录
2. 实现`Tool` trait（`definition()`和`execute()`方法）
3. 在`ToolResponse`枚举中添加变体
4. 在`lib.rs`中注册工具（4处修改）
5. 更新`tools.toml`配置

**示例工具结构**：
```rust
pub struct YourTool;

impl Tool for YourTool {
    fn definition(&self) -> ToolDefinition {
        // 定义工具参数schema
    }
    
    async fn execute(&self, function: &Function) -> ToolResponse {
        // 执行工具逻辑
    }
}
```

**6.2 扩展Provider功能**

Provider采用通道驱动架构，扩展新功能：

1. 在`ProviderCommand`枚举中添加新命令
2. 在`handle_command()`方法中添加处理逻辑
3. 如有需要，创建新的内部模块（如`timer/`、`auto_loop/`）

**6.3 集成新LLM提供商**

1. 在`api.json`中添加新的provider配置
2. Core的`read_config`模块会自动解析
3. 确保LLM API兼容OpenAI格式

**6.4 开发工作流**

```bash
# 进入Nix开发环境
nix develop

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy

# 运行示例
cargo run --example basic
```

#### 总结/建议
- 遵循现有的模块化设计模式
- 工具开发应提供完整的错误处理
- 新功能应考虑配置化和可扩展性
- 保持与现有异步架构的一致性

### 7. 贡献指南

#### 概述
如何为SynapCore项目做贡献。

#### 引用文件
- 无特定文件，基于开源项目最佳实践

#### 正文

**7.1 贡献流程**

1. **Fork仓库**：创建个人fork
2. **创建分支**：`git checkout -b feature/your-feature`
3. **开发测试**：确保代码通过所有测试
4. **提交PR**：包含清晰的描述和测试用例
5. **代码审查**：响应review意见

**7.2 代码规范**

- **Rust代码**：遵循Rustfmt和Clippy规则
- **文档**：公共API必须有文档注释
- **测试**：新功能应包含单元测试和集成测试
- **提交信息**：使用约定式提交（Conventional Commits）

**7.3 项目结构约定**

- **模块组织**：按功能划分，避免循环依赖
- **错误处理**：使用thiserror定义清晰的错误类型
- **配置管理**：所有可配置项应在配置文件中
- **日志记录**：使用tracing或log crate进行结构化日志

**7.4 待开发功能**

根据项目当前状态，以下方向需要贡献：

1. **TUI界面完善**：与Provider深度集成
2. **Flutter前端**：开发图形用户界面
3. **更多工具**：扩展工具生态系统
4. **性能优化**：内存使用和响应时间
5. **文档完善**：用户指南和API文档

#### 总结/建议
- 贡献前请先熟悉项目架构
- 小规模、专注的PR更容易被接受
- 与维护者沟通设计思路
- 测试覆盖率是质量保证的关键

---

**文档版本**: 1.0  
**最后更新**: 2026-04-29  
**项目版本**: synapcore 0.1.0  
**维护者**: Mypenfly  

*欢迎贡献代码、报告问题或提出建议！*
