use async_std::fs;
use serde_json::{Error, Number, Result, Value};

pub async fn select_config(input: Value) -> Result<Value> {
	match input {
		Value::Object(_) => parse_config(input).await,
		Value::Array(mut vec) => {
			for item in vec.iter_mut() {
				if let Value::Object(item) = item {
					if !item.contains_key("if") {
						return throw_parse_error();
					}
					let if_item = item.get("if").unwrap();

					let satisfied = parse_if_config(if_item).await?;
					if satisfied {
						if !item.contains_key("then") {
							return throw_parse_error();
						}
						let config_item = item.remove("then").unwrap();

						return parse_config(config_item).await;
					}
				} else {
					return throw_parse_error();
				}
			}
			throw_parse_error()
		}
		_ => throw_parse_error(),
	}
}

fn throw_parse_error() -> Result<Value> {
	Err(Error::io(std::io::Error::from(
		std::io::ErrorKind::InvalidData,
	)))
}

async fn parse_if_config(input: &Value) -> Result<bool> {
	if let Value::Object(if_map) = input {
		let mut satisfied = false;
		let allowed_cfgs = vec![
			"target_family".to_string(),
			"target_os".to_string(),
			"target_arch".to_string(),
			"target_endian".to_string(),
		];
		for cfg in if_map.keys() {
			if !allowed_cfgs.contains(cfg) {
				return Err(throw_parse_error().unwrap_err());
			}

			match cfg.as_str() {
				"target_family" => {
					let required_cfg = if_map.get(cfg).unwrap();
					if (required_cfg == "unix" && cfg!(target_family = "unix")) ||
						(required_cfg == "windows" && cfg!(target_family = "windows"))
					{
						satisfied = true;
					} else {
						satisfied = false;
						break;
					}
				}
				"target_os" => {
					let required_cfg = if_map.get(cfg).unwrap();
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
						satisfied = true;
					} else {
						satisfied = false;
						break;
					}
				}
				"target_arch" => {
					let required_cfg = if_map.get(cfg).unwrap();
					if (required_cfg == "x86" && cfg!(target_arch = "x86")) ||
						(required_cfg == "x86_64" && cfg!(target_arch = "x86_64")) ||
						(required_cfg == "mips" && cfg!(target_arch = "mips")) ||
						(required_cfg == "powerpc" && cfg!(target_arch = "powerpc")) ||
						(required_cfg == "powerpc64" && cfg!(target_arch = "powerpc64")) ||
						(required_cfg == "arm" && cfg!(target_arch = "arm")) ||
						(required_cfg == "aarch64" && cfg!(target_arch = "aarch64"))
					{
						satisfied = true;
					} else {
						satisfied = false;
						break;
					}
				}
				"target_endian" => {
					let required_cfg = if_map.get(cfg).unwrap();
					if (required_cfg == "little" && cfg!(target_endian = "little")) ||
						(required_cfg == "big" && cfg!(target_endian = "big"))
					{
						satisfied = true;
					} else {
						satisfied = false;
						break;
					}
				}
				_ => {
					return Err(throw_parse_error().unwrap_err());
				}
			}
		}

		return Ok(satisfied);
	} else {
		Err(throw_parse_error().unwrap_err())
	}
}

async fn parse_config(input: Value) -> Result<Value> {
	if let Value::Object(mut map) = input {
		let gotham = map.get("gotham");
		if gotham.is_none() || !gotham.unwrap().is_string() {
			return throw_parse_error();
		}
		let gotham = gotham.unwrap().as_str().unwrap().to_string();

		map.insert(
			"gotham".to_string(),
			Value::String(
				fs::canonicalize(gotham)
					.await
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			),
		);

		let modules = map.get("modules");
		if modules.is_none() || !modules.unwrap().is_string() {
			return throw_parse_error();
		}
		let modules = modules.unwrap().as_str().unwrap().to_string();

		map.insert(
			"modules".to_string(),
			Value::String(
				fs::canonicalize(modules)
					.await
					.unwrap()
					.to_str()
					.unwrap()
					.to_string(),
			),
		);

		let connection_type = map.get("connection_type");
		if connection_type.is_none() {
			return throw_parse_error();
		}
		let connection_type = connection_type.unwrap();
		if connection_type == "unix_socket" {
			if map.get("path").is_none() || !map.get("path").unwrap().is_string() {
				throw_parse_error()
			} else {
				let path = map.get("path").unwrap().as_str().unwrap().to_string();
				map.insert(
					"path".to_string(),
					Value::String(
						fs::canonicalize(path)
							.await
							.unwrap()
							.to_str()
							.unwrap()
							.to_string(),
					),
				);

				Ok(Value::Object(map))
			}
		} else if connection_type == "inet_socket" {
			if map.get("port").is_none() {
				map.insert("port".to_string(), Value::Number(Number::from(2203)));
			} else if !map.get("port").unwrap().is_number() {
				return throw_parse_error();
			}
			if map.get("bind-addr").is_none() {
				map.insert(
					"bind-addr".to_string(),
					Value::String("127.0.0.1".to_string()),
				);
			} else if !map.get("bind-addr").unwrap().is_string() {
				return throw_parse_error();
			}
			Ok(Value::Object(map))
		} else {
			throw_parse_error()
		}
	} else {
		return throw_parse_error();
	}
}
