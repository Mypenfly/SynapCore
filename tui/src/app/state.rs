#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AppState {
    #[default]
    Running,
    Stopped,
}
#[derive(Clone, Debug, Default, PartialEq)]
#[allow(dead_code)]
pub enum AppPage {
    #[default]
    StartPage,
    TaskPage,
    ChatPage,
}
