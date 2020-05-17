use super::{ConfigTarget, GuillotineConfig, RunnerConfig};
use async_std::{fs, path::Path};
use serde_json::{Error, Result};

pub async fn select_config(input: String) -> Result<RunnerConfig> {
	let envs: GuillotineConfig = serde_json::from_str(&input)?;
	if envs.config.is_some() {
		parse_config(envs.config.unwrap()).await
	} else {
		let mut default_config = None;
		for config in envs.configs.unwrap().into_iter() {
			if config.target.is_none() {
				default_config = Some(config);
				continue;
			}
			if parse_if_config(&config.target.unwrap()).await? {
				return parse_config(config.config).await;
			}
		}
		if default_config.is_none() {
			throw_parse_error()
		} else {
			parse_config(default_config.unwrap().config).await
		}
	}
}

fn throw_parse_error() -> Result<RunnerConfig> {
	Err(Error::io(std::io::Error::from(
		std::io::ErrorKind::InvalidData,
	)))
}

async fn parse_if_config(input: &ConfigTarget) -> Result<bool> {
	let mut satisfied = true;

	if let Some(required_cfg) = &input.family {
		if (required_cfg == "unix" && cfg!(target_family = "unix"))
			|| (required_cfg == "windows" && cfg!(target_family = "windows"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	if let Some(required_cfg) = &input.os {
		if (required_cfg == "windows" && cfg!(target_os = "windows"))
			|| (required_cfg == "macos" && cfg!(target_os = "macos"))
			|| (required_cfg == "ios" && cfg!(target_os = "ios"))
			|| (required_cfg == "linux" && cfg!(target_os = "linux"))
			|| (required_cfg == "android" && cfg!(target_os = "android"))
			|| (required_cfg == "freebsd" && cfg!(target_os = "freebsd"))
			|| (required_cfg == "dragonfly" && cfg!(target_os = "dragonfly"))
			|| (required_cfg == "openbsd" && cfg!(target_os = "openbsd"))
			|| (required_cfg == "netbsd" && cfg!(target_os = "netbsd"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	if let Some(required_cfg) = &input.arch {
		if (required_cfg == "x86" && cfg!(target_arch = "x86"))
			|| (required_cfg == "x86_64" && cfg!(target_arch = "x86_64"))
			|| (required_cfg == "mips" && cfg!(target_arch = "mips"))
			|| (required_cfg == "powerpc" && cfg!(target_arch = "powerpc"))
			|| (required_cfg == "powerpc64" && cfg!(target_arch = "powerpc64"))
			|| (required_cfg == "arm" && cfg!(target_arch = "arm"))
			|| (required_cfg == "aarch64" && cfg!(target_arch = "aarch64"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	if let Some(required_cfg) = &input.endian {
		if (required_cfg == "little" && cfg!(target_endian = "little"))
			|| (required_cfg == "big" && cfg!(target_endian = "big"))
		{
			satisfied &= true;
		} else {
			satisfied &= false;
		}
	}

	Ok(satisfied)
}

async fn parse_config(mut input: RunnerConfig) -> Result<RunnerConfig> {
	if input.modules.is_some() {
		let mut modules = input.modules.unwrap();
		modules.directory = fs::canonicalize(modules.directory)
			.await
			.unwrap()
			.to_str()
			.unwrap()
			.to_string();
		if input.logs.is_some() {
			input.logs = Some(
				fs::canonicalize(input.logs.unwrap())
					.await
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			);
		}
		input.modules = Some(modules);
	}

	if input.host.is_some() {
		let host = input.host.unwrap();
		host.path = fs::canonicalize(host.path)
			.await
			.unwrap()
			.to_str()
			.unwrap()
			.to_string();

		if host.connection_type == "unix_socket" {
			if host.socket_path.is_none() {
				return throw_parse_error();
			}

			let socket_path = &host.socket_path.unwrap();
			let socket_path = Path::new(socket_path);

			if !socket_path.exists().await {
				fs::write(socket_path, "").await.unwrap();
			}

			host.socket_path = Some(
				fs::canonicalize(socket_path)
					.await
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			);
		} else if host.connection_type == "inet_socket" {
			if host.port.is_none() {
				return throw_parse_error();
			}

			if host.bind_addr.is_none() {
				host.bind_addr = Some("127.0.0.1".to_string());
			}
		} else {
			return throw_parse_error();
		}

		input.host = Some(host);
	} else if input.node.is_some() {
		let node = input.node.unwrap();
		if node.connection_type == "unix_socket" {
			if node.socket_path.is_none() {
				return throw_parse_error();
			}

			let socket_path = &node.socket_path.unwrap();
			let socket_path = Path::new(socket_path);

			if !socket_path.exists().await {
				fs::write(socket_path, "").await.unwrap();
			}

			node.socket_path = Some(
				fs::canonicalize(socket_path)
					.await
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			);
		} else if node.connection_type == "inet_socket" {
			if node.port.is_none() {
				return throw_parse_error();
			}

			if node.ip.is_none() {
				return throw_parse_error();
			}
		} else {
			return throw_parse_error();
		}

		input.node = Some(node);
	} else {
		return throw_parse_error();
	}

	Ok(input)
}
