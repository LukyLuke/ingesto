use thiserror::Error;

#[derive(Error, Debug)]
pub enum Errors {
	#[error("Invalid Confiugration {0}")]
	ConfigError(String)
}
