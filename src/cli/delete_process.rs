use crate::{cli::get_juno_module_from_config, logger, models::RunnerConfig, utils::constants};

use clap::ArgMatches;
use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use juno::models::{Number, Value};
use std::collections::HashMap;

pub async fn delete_process(config: RunnerConfig, args: &ArgMatches<'_>) {
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
		.call_function(&format!("{}.deleteProcess", constants::APP_NAME), {
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
		logger::error(&format!("Error deleting process: {}", error));
		return;
	}

	let response = module
		.call_function(
			&format!("{}.listAllProcesses", constants::APP_NAME),
			HashMap::new(),
		)
		.await
		.unwrap();
	let processes = if let Value::Object(mut map) = response {
		if let Some(Value::Bool(success)) = map.remove("success") {
			if success {
				if let Some(Value::Array(processes)) = map.remove("processes") {
					processes
				} else {
					logger::error("Invalid processes key in response");
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

	// Make the looks first
	let header_format = CellFormat::builder()
		.align(Align::Center)
		.bold(true)
		.underline(true)
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
	let mut table_data = vec![Row::new(vec![
		Cell::new("ID", header_format),
		Cell::new("Name", header_format),
		Cell::new("Node", header_format),
		Cell::new("Status", header_format),
		Cell::new("Restarts", header_format),
		Cell::new("Uptime", header_format),
		Cell::new("Crashes", header_format),
		Cell::new("Created at", header_format),
	])];
	for process in processes.into_iter() {
		let process = process.as_object().unwrap();
		table_data.push(Row::new(vec![
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
			Cell::new(
				process.get("name").unwrap().as_string().unwrap(),
				Default::default(),
			),
			Cell::new(
				process.get("node").unwrap().as_string().unwrap(),
				Default::default(),
			),
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
			Cell::new(
				&super::get_duration(
					process
						.get("uptime")
						.unwrap()
						.as_number()
						.unwrap()
						.as_i64()
						.unwrap(),
				),
				Default::default(),
			),
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
			Cell::new(
				&super::get_date_time(
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
		]));
	}
	let table = Table::new(table_data, table_format);

	// Print it out
	table.unwrap().print_stdout().unwrap();
}
