use crate::{
	cli::{get_date_time, get_juno_module_from_config, get_duration},
	models::RunnerConfig,
	utils::{constants, logger},
};
use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use juno::models::{Number, Value};
use std::collections::HashMap;

pub async fn get_process_info(config: RunnerConfig, pid: u64) {
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

	module
		.initialize(
			&format!("{}-cli", constants::APP_NAME),
			constants::APP_VERSION,
			HashMap::new(),
		)
		.await
		.unwrap();
	let response = module
		.call_function(
			&format!("{}.getProcessInfo", constants::APP_NAME),
			[(String::from("moduleId"), Value::Number(Number::PosInt(pid)))]
				.iter()
				.cloned()
				.collect(),
		)
		.await
		.unwrap();
	let process = if let Value::Object(mut map) = response {
		if let Some(Value::Bool(success)) = map.remove("success") {
			if success {
				if let Some(Value::Object(process)) = map.remove("process") {
					process
				} else {
					logger::error("Invalid process key in response");
					return;
				}
			} else {
				logger::error(map.remove("error").unwrap().as_string().unwrap());
				return;
			}
		} else {
			logger::error("Invalid success key in response");
			return;
		}
	} else {
		logger::error(&format!("Expected object response. Got: {:#?}", response));
		return;
	};
	let config = if let Some(Value::Object(config)) = process.get("config") {
		config
	} else {
		logger::error("Invalid config key in response");
		return;
	};

	// Make the looks first
	let header_format = CellFormat::builder()
		.align(Align::Center)
		.bold(true)
		.build();
	let table_format = TableFormat::new(
		Border::builder()
			.top(HorizontalLine::new('┌', '┐', '┬', '─'))
			.bottom(HorizontalLine::new('└', '┘', '┴', '─'))
			.right(VerticalLine::new('│'))
			.left(VerticalLine::new('│'))
			.build(),
		Separator::builder()
			.row(None) //Use this for a line: Some(HorizontalLine::new('├', '┤', '┼', '─')))
			.column(Some(VerticalLine::new('│')))
			.build(),
	);

	// Now make the data
	let mut table_data = vec![];
	table_data.extend(vec![
		Row::new(vec![
			Cell::new("Module ID", header_format),
			Cell::new(
				&format!(
					"{}",
					process
						.get("id")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Name", header_format),
			Cell::new(
				process.get("name").unwrap().as_string().unwrap(),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Command", header_format),
			Cell::new(
				config.get("command").unwrap().as_string().unwrap(),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Intepreter", header_format),
			Cell::new(
				if let Some(Value::String(intepreter)) = config.get("intepreter") {
					intepreter.as_ref()
				} else {
					"-"
				},
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Arguments", header_format),
			Cell::new(
				&constrain_string_to(
					config
						.get("args")
						.unwrap()
						.as_array()
						.unwrap()
						.iter()
						.map(|arg| arg.as_string().unwrap().clone())
						.collect(),
					55,
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Environment", header_format),
			Cell::new(
				&constrain_string_to(
					config
						.get("envs")
						.unwrap()
						.as_object()
						.unwrap()
						.iter()
						.map(|(key, value)| format!("{}={}", key, value.as_string().unwrap()))
						.collect(),
					55,
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Log Dir", header_format),
			Cell::new(
				if let Some(Value::String(log_dir)) = process.get("logDir") {
					log_dir.as_ref()
				} else {
					"-"
				},
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Working Dir", header_format),
			Cell::new(
				process.get("workingDir").unwrap().as_string().unwrap(),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Status", header_format),
			match process.get("status").unwrap().as_string().unwrap().as_ref() {
				"running" => Cell::new(
					"running",
					CellFormat::builder()
						.foreground_color(Some(Color::Green))
						.build(),
				),
				"offline" => Cell::new(
					"offline",
					CellFormat::builder()
						.foreground_color(Some(Color::Red))
						.build(),
				),
				_ => Cell::new(
					"unknown",
					CellFormat::builder()
						.foreground_color(Some(Color::Cyan))
						.build(),
				),
			},
		]),
		Row::new(vec![
			Cell::new("Node", header_format),
			Cell::new(
				process.get("node").unwrap().as_string().unwrap(),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Uptime", header_format),
			Cell::new(
				&get_duration(
					process
						.get("uptime")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Restarts", header_format),
			Cell::new(
				&format!(
					"{}",
					process
						.get("restarts")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Crashes", header_format),
			Cell::new(
				&format!(
					"{}",
					process
						.get("crashes")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap()
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Created At", header_format),
			Cell::new(
				&get_date_time(
					process
						.get("createdAt")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap(),
				),
				Default::default(),
			),
		]),
	]);
	let table = Table::new(table_data, table_format);

	// Print it out
	table.unwrap().print_stdout().unwrap();
}

fn constrain_string_to(array: Vec<String>, max_length: usize) -> String {
	let total_string = array.join(", ");
	if total_string.len() > max_length {
		let mut total_string: String = total_string.chars().take(max_length - 3).collect();
		total_string.push_str("...");
		total_string
	} else if total_string.is_empty() {
		String::from("-")
	} else {
		total_string
	}
}
