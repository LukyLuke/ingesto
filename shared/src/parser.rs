use core::time;
use std::{sync::Arc, thread::{self, sleep}};

use tracing::info;

use crate::queue;

pub struct MessageParser<T> {
	queue: Arc<queue::MessageQueue<T>>,
}

impl<T: Send + 'static + Into<String>> MessageParser<T> {
	pub fn new(queue: Arc<queue::MessageQueue<T>>) ->Self {
		Self {
			queue,
		}
	}

	pub fn run(&self) {
		let queue = Arc::clone(&self.queue);
		thread::spawn(move || {
			loop {
				for t_msg in queue.pull_all().into_iter() {
					let msg = t_msg.into().trim().to_string();
					info!("read out and process: {}", msg);
				}

				sleep(time::Duration::from_secs_f32(2.0));
			}
		});
	}
}
