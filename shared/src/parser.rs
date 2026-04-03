use std::{sync::Arc, thread::{self}, time::{Duration, Instant}};

use tracing::{debug, info};

use crate::queue;

pub struct MessageParser<T> {
	queue: Arc<queue::MessageQueue<T>>,
	conf: queue::Queue,
}

impl<T: Send + 'static + Into<String> + From<String>> MessageParser<T> {
	pub fn new(queue: Arc<queue::MessageQueue<T>>, conf: queue::Queue) ->Self {
		Self {
			queue,
			conf,
		}
	}

	pub fn run(&self) {
		let queue = Arc::clone(&self.queue);
		let max_size = self.conf.max_size - 2; // -2 for the [] around the messages
		let max_msg = self.conf.max_messages;
		let max_time = Duration::from_secs_f32(self.conf.max_seconds as f32);

		info!(message="start processing", max_time=%max_time.as_secs_f32(), max_messages=%max_msg, max_message_size=%max_size);
		thread::spawn(move || {
			loop {
				let start = Instant::now();
				let mut msg = String::with_capacity(max_size);
				let mut count: u16 = 0;
				let mut chars:usize = 0;

				msg.push('[');
				while chars < max_size {
					let elapsed = start.elapsed();
					let remaining = max_time - elapsed;
					let q_msg = match queue.pull(remaining) {
						Some(m) => m.into().trim().to_string(),
						None => {
							info!(message="queue empty", waited=%remaining.as_secs_f32());
							break;
						}
					};

					// TODO: Parse and Structure
					let p_msg = &q_msg;

					// If the final message would be too long, close the old message and push the current one back to the front
					// But if this is the first message and that one is already too long, add it and anyways
					if (count > 0) && (chars + p_msg.chars().count() > max_size) {
						queue.push_front(q_msg.into());
						break;
					}

					if count > 0 {
						msg.push(',');
					}
					msg.push_str(&p_msg);
					chars = msg.chars().count();

					count += 1;
					if count >= max_msg {
						break;
					}
				}
				msg.push(']');

				// TODO: Send the message out
				info!(message="messages processed", count=%count, size=%chars);
				debug!(message=%msg);
			}
		});
	}
}
