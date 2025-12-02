use crate::{config::WorkspacesModuleConfig, outputs::Outputs, theme::AshellTheme};
use iced::{Element, Subscription, window::Id};

#[cfg(feature = "hyprland")]
pub mod hyprland;
#[cfg(feature = "hyprland")]
pub use hyprland::HyprlandWorkspaceManager;

#[cfg(feature = "niri")]
pub mod niri;
#[cfg(feature = "niri")]
pub use niri::NiriWorkspaceManager;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Displayed {
    Active,
    Visible,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: i32,
    pub name: String,
    pub monitor_id: Option<i128>,
    pub monitor: String,
    pub displayed: Displayed,
    pub windows: u16,
}

#[derive(Debug, Clone)]
pub enum Message {
    WorkspacesChanged,
    ChangeWorkspace(i32),
    ToggleSpecialWorkspace(i32),
    Scroll(i32),
}

pub trait WorkspaceManager {
    fn get_workspaces(config: &WorkspacesModuleConfig) -> Vec<Workspace>;
    fn create_subscription(config: &WorkspacesModuleConfig) -> Subscription<Message>;
    fn change_workspace(
        id: i32,
        config: &WorkspacesModuleConfig,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn toggle_special_workspace(workspace: &Workspace) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Workspaces<WM: WorkspaceManager> {
    config: WorkspacesModuleConfig,
    workspaces: Vec<Workspace>,
    _phantom: std::marker::PhantomData<WM>,
}

impl<WM: WorkspaceManager> Workspaces<WM> {
    pub fn new(config: WorkspacesModuleConfig) -> Self {
        let workspaces = WM::get_workspaces(&config);

        Self {
            config,
            workspaces,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::WorkspacesChanged => {
                self.workspaces = WM::get_workspaces(&self.config);
            }
            Message::ChangeWorkspace(id) => {
                if id > 0 {
                    let already_active = self
                        .workspaces
                        .iter()
                        .any(|w| w.displayed == Displayed::Active && w.id == id);

                    if !already_active {
                        if let Err(e) = WM::change_workspace(id, &self.config) {
                            log::error!("failed to dispatch workspace change: {e:?}");
                        }
                    }
                }
            }
            Message::ToggleSpecialWorkspace(id) => {
                if let Some(special) = self.workspaces.iter().find(|w| w.id == id && w.id < 0) {
                    if let Err(e) = WM::toggle_special_workspace(special) {
                        log::error!("failed to dispatch special workspace toggle: {e:?}");
                    }
                }
            }
            Message::Scroll(direction) => {
                let current_workspace = self
                    .workspaces
                    .iter()
                    .find(|w| w.displayed.eq(&Displayed::Active));
                let Some(current_id) = current_workspace.map(|w| w.id) else {
                    return;
                };

                let next_workspace = if direction > 0 {
                    self.workspaces
                        .iter()
                        .filter(|w| w.id > current_id)
                        .min_by_key(|w| w.id)
                } else {
                    self.workspaces
                        .iter()
                        .filter(|w| w.id < current_id)
                        .max_by_key(|w| w.id)
                };
                let Some(next_workspace) = next_workspace else {
                    return;
                };
                Self::update(self, Message::ChangeWorkspace(next_workspace.id));
            }
        }
    }

    pub fn view<'a>(
        &'a self,
        id: Id,
        theme: &'a AshellTheme,
        outputs: &Outputs,
    ) -> Element<'a, Message> {
        use crate::config::WorkspaceVisibilityMode;
        use iced::{
            Length, alignment,
            widget::{MouseArea, Row, button, container, text},
        };

        let monitor_name = outputs.get_monitor_name(id);

        Into::<Element<Message>>::into(
            MouseArea::new(
                Row::with_children(
                    self.workspaces
                        .iter()
                        .filter_map(|w| {
                            let show = match self.config.visibility_mode {
                                WorkspaceVisibilityMode::All => true,
                                WorkspaceVisibilityMode::MonitorSpecific => {
                                    monitor_name
                                        .unwrap_or_else(|| &w.monitor)
                                        .contains(&w.monitor)
                                        || !outputs.has_name(&w.monitor)
                                }
                                WorkspaceVisibilityMode::MonitorSpecificExclusive => monitor_name
                                    .unwrap_or_else(|| &w.monitor)
                                    .contains(&w.monitor),
                            };
                            if show {
                                let empty = w.windows == 0;

                                let color_index = if self.config.enable_virtual_desktops {
                                    // For virtual desktops, we use the workspace ID as the index
                                    Some(w.id as i128)
                                } else {
                                    // For normal workspaces, we use the monitor ID as the index
                                    w.monitor_id
                                };
                                let color = color_index.map(|i| {
                                    if w.id > 0 {
                                        theme.workspace_colors.get(i as usize).copied()
                                    } else {
                                        theme
                                            .special_workspace_colors
                                            .as_ref()
                                            .unwrap_or(&theme.workspace_colors)
                                            .get(i as usize)
                                            .copied()
                                    }
                                });

                                Some(
                                    button(
                                        container(text(w.name.as_str()).size(theme.font_size.xs))
                                            .align_x(alignment::Horizontal::Center)
                                            .align_y(alignment::Vertical::Center),
                                    )
                                    .style(theme.workspace_button_style(empty, color))
                                    .padding(if w.id < 0 {
                                        match w.displayed {
                                            Displayed::Active => [0, theme.space.md],
                                            Displayed::Visible => [0, theme.space.sm],
                                            Displayed::Hidden => [0, theme.space.xs],
                                        }
                                    } else {
                                        [0, 0]
                                    })
                                    .on_press(if w.id > 0 {
                                        Message::ChangeWorkspace(w.id)
                                    } else {
                                        Message::ToggleSpecialWorkspace(w.id)
                                    })
                                    .width(match (w.id < 0, &w.displayed) {
                                        (true, _) => Length::Shrink,
                                        (_, Displayed::Active) => {
                                            Length::Fixed(theme.space.xl as f32)
                                        }
                                        (_, Displayed::Visible) => {
                                            Length::Fixed(theme.space.lg as f32)
                                        }
                                        (_, Displayed::Hidden) => {
                                            Length::Fixed(theme.space.md as f32)
                                        }
                                    })
                                    .height(theme.space.md)
                                    .into(),
                                )
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<Element<'_, _, _>>>(),
                )
                .spacing(theme.space.xxs),
            )
            .on_scroll(move |direction| {
                let delta = match direction {
                    iced::mouse::ScrollDelta::Lines { y, .. } => y,
                    iced::mouse::ScrollDelta::Pixels { y, .. } => y,
                };

                // Scrolling down should increase workspace ID
                if delta < 0.0 {
                    Message::Scroll(1)
                } else {
                    Message::Scroll(-1)
                }
            }),
        )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        WM::create_subscription(&self.config)
    }
}
