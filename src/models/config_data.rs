use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct GuillotineConfig {
	pub version: u64,
	pub configs: Option<Vec<GuillotinePerEnvConfig>>,
	pub config: Option<RunnerConfig>,
}

#[derive(Deserialize)]
pub struct GuillotinePerEnvConfig {
	pub target: Option<ConfigTarget>,
	pub config: RunnerConfig,
}

#[derive(Deserialize)]
pub struct ConfigTarget {
	pub family: Option<String>,
	pub os: Option<String>,
	pub arch: Option<String>,
	pub endian: Option<String>,
}

// Config specific to this environment
#[derive(Deserialize, Clone)]
pub struct RunnerConfig {
	pub name: Option<String>,
	pub logs: Option<String>,
	pub host: Option<HostConfig>,
	pub node: Option<NodeConfig>,
	pub modules: Option<ModuleConfig>,
}

#[derive(Deserialize, Clone)]
pub struct HostConfig {
	pub path: String,
	pub connection_type: String,
	pub port: Option<u16>,
	pub bind_addr: Option<String>,
	pub socket_path: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct NodeConfig {
	pub connection_type: String,
	pub port: Option<u16>,
	pub ip: Option<String>,
	pub socket_path: Option<String>,
}

pub struct ModuleConfig {
	pub directory: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleRunningStatus {
	Running,
	Stopped,
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
