use std::sync::{Condvar, Mutex};
use std::collections::VecDeque;

pub struct MessageQueue<T> {
	m: Mutex<VecDeque<T>>,
	cv: Condvar
}

impl<T: Into<String>> MessageQueue<T> {
	pub fn new() -> Self {
		Self {
			m: Mutex::new(VecDeque::new()),
			cv: Condvar::new(),
		}
	}

	pub fn push(&self, val: T) {
		let mut guard = self.m.lock().unwrap();
		guard.push_back(val);
		self.cv.notify_one();
	}

	pub fn pull(&self) -> Option<T> {
		let mut guard = self.cv.wait_while(
				self.m.lock().unwrap(), |queue| queue.is_empty()
			).unwrap();
		guard.pop_front()
	}

	pub fn pull_num(&self, num: usize) -> Vec<T> {
		let mut guard = self.cv.wait_while(
				self.m.lock().unwrap(), |queue| queue.is_empty()
			).unwrap();

		let max = if num == 0 { guard.len() } else { num };
		let mut values: Vec<T> = Vec::new();
		loop {
			match guard.pop_front() {
				Some(val) => {
					values.push(val);
					if values.len() >= max {
						break;
					}
				},
				None => { break; }
			}
		}
		values
	}

	pub fn pull_all(&self) -> Vec<T> {
		self.pull_num(0)
	}

	pub fn size(&self) -> usize {
		let guard = self.m.lock().unwrap();
		guard.len()
	}
}
