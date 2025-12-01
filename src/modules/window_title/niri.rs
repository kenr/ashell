use crate::{
    config::{WindowTitleConfig, WindowTitleMode},
    utils::truncate_text,
};
use iced::{Subscription, stream::channel};
use log::{debug, error};
use std::{
    any::TypeId,
    sync::{Arc, RwLock},
};

use super::{Message, WindowManager};

pub struct NiriWindowManager;

impl WindowManager for NiriWindowManager {
    fn get_window(config: &WindowTitleConfig) -> Option<String> {
        None()
    }

    fn create_subscription() -> Subscription<Message> {
        let id = TypeId::of::<Self>();

        Subscription::run_with_id(
            id,
            channel(10, async |output| {
                let output = Arc::new(RwLock::new(output));
                loop {
                    println!("Busy!");
                }
            }),
        )
    }
}
