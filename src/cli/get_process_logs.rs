use crate::{cli::get_juno_module_from_config, logger, models::RunnerConfig, utils::constants};

use clap::ArgMatches;
use colored::Colorize;
use juno::models::{Number, Value};
use std::collections::HashMap;

pub async fn get_process_logs(config: RunnerConfig, args: &ArgMatches<'_>) {
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
		.call_function(&format!("{}.getProcessLogs", constants::APP_NAME), {
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
		logger::error(&format!("Error getting logs of process: {}", error));
		return;
	}

	let stdout_header = format!("|{}-stdout|", pid).green();
	let stderr_header = format!("|{}-stderr|", pid).red();

	println!();
	response
		.get("stdout")
		.unwrap()
		.as_string()
		.unwrap()
		.split('\n')
		.for_each(|line| {
			println!("{}: {}", stdout_header, line);
		});
	println!();
	response
		.get("stderr")
		.unwrap()
		.as_string()
		.unwrap()
		.split('\n')
		.for_each(|line| {
			println!("{}: {}", stderr_header, line);
		});
	println!();
}
