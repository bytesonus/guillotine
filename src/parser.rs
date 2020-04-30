use async_std::{fs, path::Path};
use serde_derive::Deserialize;
use serde_json::{Error, Result};

#[derive(Deserialize)]
pub struct ConfigData {
	pub version: String,
	pub configs: Option<Vec<EnvConfig>>,
	pub config: Option<ConfigValue>,
}

#[derive(Deserialize)]
pub struct EnvConfig {
	pub env: EnvRequirements,
	pub config: ConfigValue,
}

#[derive(Deserialize)]
pub struct EnvRequirements {
	pub target_family: Option<String>,
	pub target_os: Option<String>,
	pub target_arch: Option<String>,
	pub target_endian: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ConfigValue {
	pub juno: JunoConfig,
	pub modules: String,
}

#[derive(Deserialize, Clone)]
pub struct JunoConfig {
	pub path: String,
	pub connection_type: String,
	pub port: Option<u16>,
	pub bind_addr: Option<String>,
	pub socket_path: Option<String>,
}

pub async fn select_config(input: String) -> Result<ConfigValue> {
	let envs: ConfigData = serde_json::from_str(&input)?;
	if envs.config.is_some() {
		parse_config(envs.config.unwrap()).await
	} else {
		for config in envs.configs.unwrap().into_iter() {
			if parse_if_config(&config.env).await? {
				return parse_config(config.config).await;
			}
		}
		throw_parse_error()
	}
}

fn throw_parse_error() -> Result<ConfigValue> {
	Err(Error::io(std::io::Error::from(
		std::io::ErrorKind::InvalidData,
	)))
}

async fn parse_if_config(input: &EnvRequirements) -> Result<bool> {
	let mut satisfied = true;

	if let Some(required_cfg) = &input.target_family {
		if (required_cfg == "unix" && cfg!(target_family = "unix")) ||
			(required_cfg == "windows" && cfg!(target_family = "windows"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	if let Some(required_cfg) = &input.target_os {
		if (required_cfg == "windows" && cfg!(target_os = "windows")) ||
			(required_cfg == "macos" && cfg!(target_os = "macos")) ||
			(required_cfg == "ios" && cfg!(target_os = "ios")) ||
			(required_cfg == "linux" && cfg!(target_os = "linux")) ||
			(required_cfg == "android" && cfg!(target_os = "android")) ||
			(required_cfg == "freebsd" && cfg!(target_os = "freebsd")) ||
			(required_cfg == "dragonfly" && cfg!(target_os = "dragonfly")) ||
			(required_cfg == "openbsd" && cfg!(target_os = "openbsd")) ||
			(required_cfg == "netbsd" && cfg!(target_os = "netbsd"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	if let Some(required_cfg) = &input.target_arch {
		if (required_cfg == "x86" && cfg!(target_arch = "x86")) ||
			(required_cfg == "x86_64" && cfg!(target_arch = "x86_64")) ||
			(required_cfg == "mips" && cfg!(target_arch = "mips")) ||
			(required_cfg == "powerpc" && cfg!(target_arch = "powerpc")) ||
			(required_cfg == "powerpc64" && cfg!(target_arch = "powerpc64")) ||
			(required_cfg == "arm" && cfg!(target_arch = "arm")) ||
			(required_cfg == "aarch64" && cfg!(target_arch = "aarch64"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	if let Some(required_cfg) = &input.target_endian {
		if (required_cfg == "little" && cfg!(target_endian = "little")) ||
			(required_cfg == "big" && cfg!(target_endian = "big"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	Ok(satisfied)
}

async fn parse_config(mut input: ConfigValue) -> Result<ConfigValue> {
	input.juno.path = fs::canonicalize(input.juno.path)
		.await
		.unwrap()
		.to_str()
		.unwrap()
		.to_string();

	input.modules = fs::canonicalize(input.modules)
		.await
		.unwrap()
		.to_str()
		.unwrap()
		.to_string();

	if input.juno.connection_type == "unix_socket" {
		if input.juno.socket_path.is_none() {
			return throw_parse_error();
		}
		let socket_path = &input.juno.socket_path.unwrap();
		let socket_path = Path::new(socket_path);

		if !socket_path.exists().await {
			fs::write(socket_path, "").await.unwrap();
		}

		input.juno.socket_path = Some(
			fs::canonicalize(socket_path)
				.await
				.unwrap()
				.to_str()
				.unwrap()
				.to_string(),
		);

		Ok(input)
	} else if input.juno.connection_type == "inet_socket" {
		if input.juno.port.is_none() {
			input.juno.port = Some(2203);
		}

		if input.juno.bind_addr.is_none() {
			input.juno.bind_addr = Some("127.0.0.1".to_string());
		}

		Ok(input)
	} else {
		throw_parse_error()
	}
}
