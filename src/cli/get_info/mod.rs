mod get_module_info;
mod get_process_info;

use crate::{models::RunnerConfig, utils::logger};
use clap::ArgMatches;

pub async fn get_info(config: RunnerConfig, args: &ArgMatches<'_>) {
	let module_id = args.value_of("pid");
	if module_id.is_none() {
		logger::error("No pid supplied!");
		return;
	}
	let module_id = module_id.unwrap();
	if let Ok(pid) = module_id.parse::<u64>() {
		get_process_info::get_process_info(config, pid).await;
	} else {
		get_module_info::get_module_info(config, module_id).await;
	}
}
