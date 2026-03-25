pub mod config;

use std::{io::Read, net::{Ipv4Addr, SocketAddrV4, TcpListener, UdpSocket}, path::PathBuf, sync::Arc};

use anyhow::{Context, anyhow};
use clap::{Arg, Command, builder::{PathBufValueParser}};
use shared::{self, init_logging, parser::MessageParser, queue::MessageQueue};
use tracing::{debug, error, info};

const MAX_PACKET_SIZE: usize = 67 * 1024;

fn main() {
	init_logging();

	let conf_file = match usage() {
		Ok(f) => f,
		Err(e) => {
			error!(%e);
			return
		}
	};

	let r_conf: anyhow::Result<config::Config> = shared::load_config(conf_file);
	let conf = match r_conf {
		Ok(c) => c.receiver,
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	MessageParser::<String>::new(Arc::clone(&queue)).run();

	let res = match conf.listen.kind.as_str() {
		"udp"|"UDP" => start_udp_listener(conf.listen, &queue),
		"tcp"|"TCP" => start_tcp_listener(conf.listen, &queue),
		_ => Err(anyhow!("Invalid listener kind: {}; Must be udp, UDP, tcp, TCP", conf.listen.kind.as_str()))
	};
	match res {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

fn usage() -> anyhow::Result<PathBuf> {
	let matches = Command::new("Network LogStream Parser")
		.about("Listens on UDP or TCP for incomming packages, parses the data absed on the configuration and forwards it as structured data.")
		.arg(Arg::new("config_file")
			.default_value("config.toml")
			.value_parser(PathBufValueParser::default())
			.short('c')
			.long("config")
			.help("Configuration file to use"))
		.get_matches();

	let f: &PathBuf = matches.get_one("config_file").unwrap();
	return Ok(f.to_path_buf())
}

fn start_udp_listener(conf: config::Server, queue: &shared::queue::MessageQueue<String>) -> anyhow::Result<()> {
	let socket = UdpSocket::bind(conf.get_address()).with_context(|| format!("binding to {}", conf.get_address()))?;
	let mut buffer: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];
	info!(message="listener started", prococol="UDP", address=%conf.address, port=%conf.port);

	loop {
		let (b_recv, src_addr) = socket.recv_from(&mut buffer)?;
		let data = String::from_utf8_lossy(&buffer[..b_recv]);
		debug!(message="data received", src_addr=%src_addr, size=b_recv, data=%data);

		queue.push(data.into_owned());
	}
}

fn start_tcp_listener(conf: config::Server, queue: &shared::queue::MessageQueue<String>) -> anyhow::Result<()> {
	let socket = TcpListener::bind(conf.get_address()).with_context(|| format!("binding to {}", conf.get_address()))?;
	let mut buffer: [u8; MAX_PACKET_SIZE] = [0; MAX_PACKET_SIZE];
	info!(message="listener started", prococol="TCP", address=%conf.address, port=%conf.port);

	for stream in socket.incoming() {
		match stream {
			Ok(mut stream) => {
				let b_recv = match stream.read(&mut buffer) {
					Ok(size) => size,
					Err(e) => {
						error!("{}", e);
						0
					},
				};
				let src_addr = match stream.peer_addr() {
					Ok(a) => a,
					Err(e) => {
						error!("{}", e);
						std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0))
					},
				};
				let data = String::from_utf8_lossy(&buffer[..b_recv]);
				debug!(message="data received", src_addr=%src_addr, size=b_recv, data=%data);

				queue.push(data.into_owned());
			},
			Err(e) => error!("failed to establish a connection: {}", e)
		};
	}
	Ok(())
}
