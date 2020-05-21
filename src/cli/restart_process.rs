use crate::{cli::get_juno_module_from_config, logger, models::RunnerConfig, utils::constants};

use clap::ArgMatches;
use juno::models::{Number, Value};
use std::collections::HashMap;

pub async fn restart_process(config: RunnerConfig, args: &ArgMatches<'_>) {
	let result = get_juno_module_from_config(&config);
	let mut module = if let Ok(module) = result {
		module
	} else {
		logger::error(if let Err(err) = result {
			err
		} else {
			return;
		});
		return;
	};
	let pid = args.value_of("pid");
	if pid.is_none() {
		logger::error("No pid supplied!");
		return;
	}
	let pid = pid.unwrap().parse::<u64>();
	if pid.is_err() {
		logger::error("Pid supplied is not a number!");
		return;
	}
	let pid = pid.unwrap();

	module
		.initialize(
			&format!("{}-cli", constants::APP_NAME),
			constants::APP_VERSION,
			HashMap::new(),
		)
		.await
		.unwrap();

	let response = module
		.call_function(&format!("{}.restartProcess", constants::APP_NAME), {
			let mut map = HashMap::new();
			map.insert(String::from("moduleId"), Value::Number(Number::PosInt(pid)));
			map
		})
		.await
		.unwrap();

	if !response.is_object() {
		logger::error(&format!("Expected object response. Got {:?}", response));
		return;
	}
	let response = response.as_object().unwrap();

	let success = response.get("success").unwrap();
	if !success.as_bool().unwrap() {
		let error = response.get("error").unwrap().as_string().unwrap();
		logger::error(&format!("Error restarting process: {}", error));
		return;
	}

	super::list_all_processes(config).await;
}
