use crate::types::Server;

impl Server {
	/// Returns the address to listen on: IP:PORT
	pub fn get_address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}
}
