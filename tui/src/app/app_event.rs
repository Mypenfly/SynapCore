use ratatui::crossterm::event::KeyEvent;
use synapcore_provider::ProviderResponse;

/// 应用事件枚举
#[derive(Debug)]
pub enum AppEvent {
    /// 键盘事件
    Key(KeyEvent),

    /// Provider响应
    ProviderResponse(ProviderResponse),

    /// 用户提交输入
    InputSubmitted(String),

    /// 滚动事件（正数向上，负数向下）
    Scroll(i32),

    /// 详情显示切换
    DetailToggle(usize), // chunk索引

    /// 生成状态变化
    Generating(bool),

    /// 定时器滴答（用于动画等）
    Tick,

    /// 窗口大小变化
    Resize(u16, u16),

    /// 退出应用
    Exit,
}
