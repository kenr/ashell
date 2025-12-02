use super::{Message, Workspace, WorkspaceManager};
use crate::config::WorkspacesModuleConfig;
use iced::{Subscription, stream::channel};
use std::future::pending;
use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};
use tokio::task;

#[derive(Debug, Clone)]
pub struct VirtualDesktop {
    pub active: bool,
    pub windows: u16,
}

pub struct NiriWorkspaceManager;

impl WorkspaceManager for NiriWorkspaceManager {
    fn get_workspaces(_config: &WorkspacesModuleConfig) -> Vec<Workspace> {
        vec![]
    }

    fn create_subscription(_config: &WorkspacesModuleConfig) -> Subscription<Message> {
        let id = TypeId::of::<Self>();

        Subscription::run_with_id(
            id,
            channel(10, async |output| {
                let _output = Arc::new(RwLock::new(output));
                loop {
                    task::spawn(async move {
                        pending::<()>().await;
                    })
                    .await
                    .unwrap();
                }
            }),
        )
    }

    fn change_workspace(
        _id: i32,
        _config: &WorkspacesModuleConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn toggle_special_workspace(_workspace: &Workspace) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
