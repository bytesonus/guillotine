use crate::{logger, models::GuillotineSpecificConfig, utils::constants};

use clap::ArgMatches;
use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use juno::{models::Value, JunoModule};
use std::collections::HashMap;

pub async fn get_module_info(config: GuillotineSpecificConfig, args: &ArgMatches<'_>) {
	let mut module = if config.juno.connection_type == "unix_socket" {
		let socket_path = config.juno.socket_path.as_ref().unwrap();
		JunoModule::from_unix_socket(&socket_path)
	} else {
		let port = config.juno.port.as_ref().unwrap();
		let bind_addr = config.juno.bind_addr.as_ref().unwrap();
		JunoModule::from_inet_socket(&bind_addr, *port)
	};
	let module_id = args.value_of("pid");
	if module_id.is_none() {
		logger::error("No pid supplied!");
		return;
	}
	let module_id = module_id.unwrap();

	module
		.initialize(
			&format!("{}-cli", constants::APP_NAME),
			constants::APP_VERSION,
			HashMap::new(),
		)
		.await
		.unwrap();
	let module = module
		.call_function(
			"juno.getModuleInfo",
			[(
				String::from("moduleId"),
				Value::String(String::from(module_id)),
			)]
			.iter()
			.cloned()
			.collect(),
		)
		.await
		.unwrap();
	if module.is_null() {
		logger::error(&format!(
			"Couldn't find any module with moduleId: {}",
			module_id
		));
		return;
	}
	let module = module.as_object().unwrap();

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
				module.get("moduleId").unwrap().as_string().unwrap(),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Version", header_format),
			Cell::new(
				module.get("version").unwrap().as_string().unwrap(),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Registered", header_format),
			match module.get("registered").unwrap().as_bool().unwrap() {
				true => Cell::new(
					"active",
					CellFormat::builder()
						.foreground_color(Some(Color::Green))
						.build(),
				),
				false => Cell::new(
					"inactive",
					CellFormat::builder()
						.foreground_color(Some(Color::Cyan))
						.build(),
				),
			},
		]),
		Row::new(vec![
			Cell::new("Dependencies", header_format),
			Cell::new(
				&constrain_string_to(
					module
						.get("dependencies")
						.unwrap()
						.as_object()
						.unwrap()
						.iter()
						.map(|(dep, _)| dep.clone())
						.collect(),
					55,
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Declared Functions", header_format),
			Cell::new(
				&constrain_string_to(
					module
						.get("declaredFunctions")
						.unwrap()
						.as_array()
						.unwrap()
						.iter()
						.map(|function| function.as_string().unwrap().clone())
						.collect(),
					55,
				),
				Default::default(),
			),
		]),
		Row::new(vec![
			Cell::new("Registered Hooks", header_format),
			Cell::new(
				&constrain_string_to(
					module
						.get("registeredHooks")
						.unwrap()
						.as_array()
						.unwrap()
						.iter()
						.map(|function| function.as_string().unwrap().clone())
						.collect(),
					55,
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
