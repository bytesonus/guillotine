use crate::{cli::get_juno_module_from_config, logger, models::RunnerConfig, utils::constants};

use cli_table::{
	format::{
		Align, Border, CellFormat, Color, HorizontalLine, Separator, TableFormat, VerticalLine,
	},
	Cell, Row, Table,
};
use juno::models::Value;
use std::collections::HashMap;

pub async fn get_module_info(config: RunnerConfig, module_id: &str) {
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
	let table_data = vec![
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
	];
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
