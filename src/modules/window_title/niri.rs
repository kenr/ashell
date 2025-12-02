use crate::config::WindowTitleConfig;
use iced::{Subscription, stream::channel};
use std::future::pending;
use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};
use tokio::task;

use super::{Message, WindowManager};

pub struct NiriWindowManager;

impl WindowManager for NiriWindowManager {
    fn get_window(_config: &WindowTitleConfig) -> Option<String> {
        None
    }

    fn create_subscription() -> Subscription<Message> {
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
}
