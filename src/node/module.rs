use super::process::Process;
use crate::models::ModuleRunnerConfig;
use async_std::{
	fs,
	path::{Path, PathBuf},
};

pub async fn get_module_from_path(path: &str, log_dir: Option<String>) -> Option<Process> {
	let mut path = PathBuf::from(path);
	if !path.exists().await {
		return None;
	}

	// If path is a folder, get the module.json file
	if path.is_dir().await {
		path.push("module.json");
		if !path.exists().await {
			return None;
		}
	}

	// If a random file was given, say fuck off
	if !path.ends_with("module.json") {
		return None;
	}

	let module_json_contents = fs::read_to_string(&path).await;
	if module_json_contents.is_err() {
		return None;
	}
	let module_json_contents = module_json_contents.unwrap();

	let config: Result<ModuleRunnerConfig, serde_json::Error> =
		serde_json::from_str(&module_json_contents);

	if config.is_err() {
		return None;
	}
	let config = config.unwrap();

	let working_dir = path.parent().unwrap().to_str().unwrap().to_owned();

	Some(if let Some(log_dir) = log_dir {
		let main_dir = Path::new(&log_dir);
		if !main_dir.exists().await {
			fs::create_dir(&main_dir).await.unwrap();
		}

		let sub_dir = main_dir.join(&config.name);
		if !sub_dir.exists().await {
			fs::create_dir(&sub_dir).await.unwrap();
		}
		let log_sub_dir = sub_dir.to_str().unwrap().to_owned();
		Process::new(config, Some(log_sub_dir), working_dir)
	} else {
		Process::new(config, None, working_dir)
	})
}
