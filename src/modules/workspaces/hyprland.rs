use super::{Displayed, Message, Workspace, WorkspaceManager};
use crate::config::WorkspacesModuleConfig;
use hyprland::{
    dispatch::MonitorIdentifier,
    event_listener::AsyncEventListener,
    shared::{HyprData, HyprDataActive, HyprDataVec},
};
use iced::{Subscription, stream::channel};
use itertools::Itertools;
use log::{debug, error};
use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
pub struct VirtualDesktop {
    pub active: bool,
    pub windows: u16,
}

pub struct HyprlandWorkspaceManager;

impl WorkspaceManager for HyprlandWorkspaceManager {
    fn get_workspaces(config: &WorkspacesModuleConfig) -> Vec<Workspace> {
        let active = hyprland::data::Workspace::get_active().ok();
        let monitors = hyprland::data::Monitors::get()
            .map(|m| m.to_vec())
            .unwrap_or_default();
        let workspaces = hyprland::data::Workspaces::get()
            .map(|w| w.to_vec())
            .unwrap_or_default();

        // in some cases we can get duplicate workspaces, so we need to deduplicate them
        let workspaces: Vec<_> = workspaces.into_iter().unique_by(|w| w.id).collect();

        // We need capacity for at least all the existing entries.
        let mut result: Vec<Workspace> = Vec::with_capacity(workspaces.len());

        let (special, normal): (Vec<_>, Vec<_>) = workspaces.into_iter().partition(|w| w.id < 0);

        // map special workspaces
        for w in special.iter() {
            // Special workspaces are active if they are assigned to any monitor.
            // Currently a special and normal workspace can be active at the same time on the same monitor.
            let active = monitors.iter().any(|m| m.special_workspace.id == w.id);
            result.push(Workspace {
                id: w.id,
                name: w
                    .name
                    .split(":")
                    .last()
                    .map_or_else(|| "".to_string(), |s| s.to_owned()),
                monitor_id: w.monitor_id,
                monitor: w.monitor.clone(),
                displayed: if active {
                    Displayed::Active
                } else {
                    Displayed::Hidden
                },
                windows: w.windows,
            });
        }

        if config.enable_virtual_desktops {
            let monitor_count = monitors.len();
            let mut virtual_desktops: HashMap<i32, VirtualDesktop> = HashMap::new();

            // map normal workspaces
            for w in normal.iter() {
                // Calculate the virtual desktop ID based on the workspace ID and the number of workspaces per virtual desktop
                let vdesk_id = ((w.id - 1) / monitor_count as i32) + 1;

                if let Some(vdesk) = virtual_desktops.get_mut(&vdesk_id) {
                    vdesk.windows += w.windows;
                    vdesk.active = vdesk.active || Some(w.id) == active.as_ref().map(|a| a.id);
                } else {
                    virtual_desktops.insert(
                        vdesk_id,
                        VirtualDesktop {
                            active: Some(w.id) == active.as_ref().map(|a| a.id),
                            windows: w.windows,
                        },
                    );
                }
            }

            // Add virtual desktops to the result as workspaces
            virtual_desktops.into_iter().for_each(|(id, vdesk)| {
                // Try to get a name from the config, default to ID
                let idx = (id - 1) as usize;
                let display_name = config
                    .workspace_names
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| id.to_string());
                let active = if vdesk.active {
                    Displayed::Active
                } else {
                    Displayed::Hidden
                };
                result.push(Workspace {
                    id,
                    name: display_name,
                    monitor_id: None,
                    monitor: "".to_string(),
                    displayed: active,
                    windows: vdesk.windows,
                });
            });
        } else {
            // map normal workspaces
            for w in normal.iter() {
                let display_name = if w.id > 0 {
                    let idx = (w.id - 1) as usize;
                    config
                        .workspace_names
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| w.id.to_string())
                } else {
                    w.name.clone()
                };
                let active = active.as_ref().is_some_and(|a| a.id == w.id);
                let visible = monitors.iter().any(|m| m.active_workspace.id == w.id);
                result.push(Workspace {
                    id: w.id,
                    name: display_name,
                    monitor_id: w.monitor_id,
                    monitor: w.monitor.clone(),
                    displayed: match (active, visible) {
                        (true, _) => Displayed::Active,
                        (false, true) => Displayed::Visible,
                        (false, false) => Displayed::Hidden,
                    },
                    windows: w.windows,
                });
            }
        }

        if !config.enable_workspace_filling || normal.is_empty() {
            // nothing more to do, early return
            result.sort_by_key(|w| w.id);
            return result;
        };

        // To show workspaces that don't exist in Hyprland we need to create fake ones
        let existing_ids = result.iter().map(|w| w.id).collect_vec();
        let mut max_id = *existing_ids
            .iter()
            .filter(|&&id| id > 0) // filter out special workspaces
            .max()
            .unwrap_or(&0);
        if let Some(max_workspaces) = config.max_workspaces
            && max_workspaces > max_id as u32
        {
            max_id = max_workspaces as i32;
        }
        let missing_ids: Vec<i32> = (1..=max_id)
            .filter(|id| !existing_ids.contains(id))
            .collect();

        // Rust could do reallocs for us, but here we know how many more space we need, so can do better
        result.reserve(missing_ids.len());

        for id in missing_ids {
            let display_name = if id > 0 {
                let idx = (id - 1) as usize;
                config
                    .workspace_names
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| id.to_string())
            } else {
                id.to_string()
            };
            result.push(Workspace {
                id,
                name: display_name,
                monitor_id: None,
                monitor: "".to_string(),
                displayed: Displayed::Hidden,
                windows: 0,
            });
        }

        result.sort_by_key(|w| w.id);

        result
    }

    fn create_subscription(config: &WorkspacesModuleConfig) -> Subscription<Message> {
        let id = TypeId::of::<Self>();
        let enable_workspace_filling = config.enable_workspace_filling;

        Subscription::run_with_id(
            (id, enable_workspace_filling),
            channel(10, async move |output| {
                let output = Arc::new(RwLock::new(output));
                loop {
                    let mut event_listener = AsyncEventListener::new();

                    event_listener.add_workspace_added_handler({
                        let output = output.clone();
                        move |e| {
                            debug!("workspace added: {e:?}");
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::WorkspacesChanged)
                                        .expect("error getting workspaces: workspace added event");
                                }
                            })
                        }
                    });

                    event_listener.add_workspace_changed_handler({
                        let output = output.clone();
                        move |e| {
                            debug!("workspace changed: {e:?}");
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::WorkspacesChanged)
                                        .expect("error getting workspaces: workspace change event");
                                }
                            })
                        }
                    });

                    event_listener.add_workspace_deleted_handler({
                        let output = output.clone();
                        move |e| {
                            debug!("workspace deleted: {e:?}");
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output.try_send(Message::WorkspacesChanged).expect(
                                        "error getting workspaces: workspace destroy event",
                                    );
                                }
                            })
                        }
                    });

                    event_listener.add_workspace_moved_handler({
                        let output = output.clone();
                        move |e| {
                            debug!("workspace moved: {e:?}");
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::WorkspacesChanged)
                                        .expect("error getting workspaces: workspace moved event");
                                }
                            })
                        }
                    });

                    event_listener.add_changed_special_handler({
                        let output = output.clone();
                        move |e| {
                            debug!("special workspace changed: {e:?}");
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output.try_send(Message::WorkspacesChanged).expect(
                                        "error getting workspaces: special workspace change event",
                                    );
                                }
                            })
                        }
                    });

                    event_listener.add_special_removed_handler({
                        let output = output.clone();
                        move |e| {
                            debug!("special workspace removed: {e:?}");
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output.try_send(Message::WorkspacesChanged).expect(
                                        "error getting workspaces: special workspace removed event",
                                    );
                                }
                            })
                        }
                    });

                    event_listener.add_window_closed_handler({
                        let output = output.clone();
                        move |_| {
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::WorkspacesChanged)
                                        .expect("error getting workspaces: window close event");
                                }
                            })
                        }
                    });

                    event_listener.add_window_opened_handler({
                        let output = output.clone();
                        move |_| {
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::WorkspacesChanged)
                                        .expect("error getting workspaces: window open event");
                                }
                            })
                        }
                    });

                    event_listener.add_window_moved_handler({
                        let output = output.clone();
                        move |_| {
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output
                                        .try_send(Message::WorkspacesChanged)
                                        .expect("error getting workspaces: window moved event");
                                }
                            })
                        }
                    });

                    event_listener.add_active_monitor_changed_handler({
                        let output = output.clone();
                        move |_| {
                            let output = output.clone();
                            Box::pin(async move {
                                if let Ok(mut output) = output.write() {
                                    output.try_send(Message::WorkspacesChanged).expect(
                                        "error getting workspaces: active monitor change event",
                                    );
                                }
                            })
                        }
                    });

                    let res = event_listener.start_listener_async().await;

                    if let Err(e) = res {
                        error!("restarting workspaces listener due to error: {e:?}");
                    }
                }
            }),
        )
    }

    fn change_workspace(
        id: i32,
        config: &WorkspacesModuleConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("changing workspace to: {id}");
        let res = if config.enable_virtual_desktops {
            let id_str = id.to_string();
            hyprland::dispatch::Dispatch::call(hyprland::dispatch::DispatchType::Custom(
                "vdesk", &id_str,
            ))
        } else {
            hyprland::dispatch::Dispatch::call(hyprland::dispatch::DispatchType::Workspace(
                hyprland::dispatch::WorkspaceIdentifierWithSpecial::Id(id),
            ))
        };

        res.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn toggle_special_workspace(workspace: &Workspace) -> Result<(), Box<dyn std::error::Error>> {
        debug!("toggle special workspace: {}", workspace.id);
        let res =
            hyprland::dispatch::Dispatch::call(hyprland::dispatch::DispatchType::FocusMonitor(
                MonitorIdentifier::Id(workspace.monitor_id.unwrap_or_default()),
            ))
            .and_then(|_| {
                hyprland::dispatch::Dispatch::call(
                    hyprland::dispatch::DispatchType::ToggleSpecialWorkspace(Some(
                        workspace.name.clone(),
                    )),
                )
            });

        res.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}
