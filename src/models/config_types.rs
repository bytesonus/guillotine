use serde_derive::Deserialize;

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

#[derive(Debug, Clone)]
pub enum ModuleRunningStatus {
	Running,
	Offline,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleConfig {
	pub name: String,
	pub command: String,
	pub interpreter: Option<String>,
	pub args: Option<Vec<String>>,
	pub envs: Option<Vec<(String, String)>>,
}

impl ModuleConfig {
	pub fn juno_default(path: String, args: Vec<String>) -> Self {
		ModuleConfig {
			name: "Juno".to_string(),
			command: path,
			interpreter: None,
			args: Some(args),
			envs: None,
		}
	}
}
