use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct GuillotineConfig {
	pub version: String,
	pub configs: Option<Vec<GuillotinePerEnvConfig>>,
	pub config: Option<GuillotineSpecificConfig>,
}

#[derive(Deserialize)]
pub struct GuillotinePerEnvConfig {
	pub env: EnvRequirements,
	pub config: GuillotineSpecificConfig,
}

#[derive(Deserialize)]
pub struct EnvRequirements {
	pub target_family: Option<String>,
	pub target_os: Option<String>,
	pub target_arch: Option<String>,
	pub target_endian: Option<String>,
}

// Config specific to this environment
#[derive(Deserialize, Clone)]
pub struct GuillotineSpecificConfig {
	pub juno: JunoConfig,
	pub modules: Option<GuillotineModuleConfig>,
}

#[derive(Deserialize, Clone)]
pub struct JunoConfig {
	pub path: String,
	pub connection_type: String,
	pub port: Option<u16>,
	pub bind_addr: Option<String>,
	pub socket_path: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct GuillotineModuleConfig {
	pub path: String,
	pub logs: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ModuleRunningStatus {
	Running,
	Offline,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModuleRunnerConfig {
	pub name: String,
	pub command: String,
	pub interpreter: Option<String>,
	pub args: Option<Vec<String>>,
	pub envs: Option<Vec<(String, String)>>,
}

impl ModuleRunnerConfig {
	pub fn juno_default(path: String, args: Vec<String>) -> Self {
		ModuleRunnerConfig {
			name: "Juno".to_string(),
			command: path,
			interpreter: None,
			args: Some(args),
			envs: None,
		}
	}
}
