pub mod config;

use std::{
	fs::{self, File, metadata}, io::{BufRead, BufReader, Seek, SeekFrom}, sync::{Arc, mpsc}, time::Duration
};
use notify::{self, RecursiveMode, Watcher, Event, Result};

use anyhow::{Context};
use shared::{self, init_logging, usage, parser::MessageParser, queue::MessageQueue};
use tracing::{debug, error, info};

fn main() {
	init_logging();

	let conf_file = match usage() {
		Ok(file) => file,
		Err(e) => {
			error!(%e);
			return
		}
	};

	let r_conf: anyhow::Result<config::Config> = shared::load_config(conf_file);
	let conf = match r_conf {
		Ok(c) => Arc::new(c.reader),
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	MessageParser::<String>::new(queue.clone(), conf.queue).run();

	let conf_file = &conf.file;
	let res = match conf.file.follow {
		true => start_follow_listener(conf_file, queue.clone()),
		false => start_full_listener(conf_file, queue.clone()),
	};
	match res {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

fn start_follow_listener(conf: &config::File, queue: Arc<shared::queue::MessageQueue<String>>) -> anyhow::Result<()> {
	// open the file and set the position to the end of the file
	let mut file = File::open(&conf.path)?;
	let mut pos = metadata(&conf.path)?.len();
	info!(message="listener started", file=conf.path.to_str(), position=%pos);

	let (tx, rx) = mpsc::channel::<Result<Event>>();
	let mut watcher = notify::recommended_watcher(tx)?;
	watcher.watch(conf.path.as_ref(), RecursiveMode::NonRecursive)?;

	for res in rx {
		match res {
			Ok(_event) => {
				// No new lines
				if file.metadata()?.len() == pos {
					continue;
				}

				// read from last position to the end
				file.seek(SeekFrom::Start(pos))?;
				pos = file.metadata()?.len();
				let reader = BufReader::new(&file);
				for line in reader.lines() {
					let data = line?;
					debug!(message="data received", data=%data);
					queue.push(data);
				}
			},
			Err(e) => {
				error!("{:#?}", e)
			}
		}
	}

	Ok(())
}

fn start_full_listener(conf: &config::File, queue: Arc<shared::queue::MessageQueue<String>>) -> anyhow::Result<()> {
	let interval = Duration::from_secs_f32(conf.interval);
	info!(message="listener started", file=conf.path.to_str(), interval=%conf.interval);
	loop {
		let content = fs::read_to_string(&conf.path).with_context(|| format!("reading file {}", &conf.path.display()))?;
		for line in content.lines() {
			debug!(message="data received", data=%line);
			queue.push(line.to_string());
		}
		std::thread::sleep(interval);
	}
}
