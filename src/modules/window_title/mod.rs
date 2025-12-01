use crate::{config::WindowTitleConfig, theme::AshellTheme};
use iced::{
    Element, Subscription,
    widget::{container, text},
};

#[cfg(feature = "hyprland")]
pub mod hyprland;

#[cfg(feature = "hyprland")]
pub use hyprland::HyprlandWindowManager;

#[cfg(feature = "niri")]
pub mod niri;

#[cfg(feature = "niri")]
pub use niri::NiriWindowManager;

#[derive(Debug, Clone)]
pub enum Message {
    TitleChanged,
}

pub trait WindowManager {
    fn get_window(config: &WindowTitleConfig) -> Option<String>;
    fn create_subscription() -> Subscription<Message>;
}

pub struct WindowTitle<WM: WindowManager> {
    config: WindowTitleConfig,
    value: Option<String>,
    _phantom: std::marker::PhantomData<WM>,
}

impl<WM: WindowManager> WindowTitle<WM> {
    pub fn new(config: WindowTitleConfig) -> Self {
        let init = WM::get_window(&config);

        Self {
            value: init,
            config,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::TitleChanged => {
                self.value = WM::get_window(&self.config);
            }
        }
    }

    pub fn get_value(&self) -> Option<String> {
        self.value.clone()
    }

    pub fn view(&'_ self, theme: &AshellTheme, title: String) -> Element<'_, Message> {
        container(
            text(title.to_string())
                .size(theme.font_size.sm)
                .wrapping(text::Wrapping::None),
        )
        .clip(true)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        WM::create_subscription()
    }
}
