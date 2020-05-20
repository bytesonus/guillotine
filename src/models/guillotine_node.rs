use crate::models::ProcessData;

#[derive(Debug, Clone)]
pub struct GuillotineNode {
	pub name: String,
	pub processes: Vec<ProcessData>,
	pub connected: bool,
}

impl GuillotineNode {
	pub fn get_process_by_name(&self, name: &str) -> Option<&ProcessData> {
		return self
			.processes
			.iter()
			.find(|process| process.config.name == name);
	}

	pub fn get_process_by_id(&self, id: u64) -> Option<&ProcessData> {
		return self
			.processes
			.iter()
			.find(|process| process.module_id == id);
	}

	pub fn register_process(&mut self, process_data: ProcessData) {
		let position = self
			.processes
			.iter()
			.position(|process| process.config.name == process_data.config.name);
		// If a process with the same name exists, replace the existing value
		if let Some(position) = position {
			// Only update the values that can change. Retain the remaining values
			self.processes[position].log_dir = process_data.log_dir;
			self.processes[position].working_dir = process_data.working_dir;
			self.processes[position].config = process_data.config;
			self.processes[position].status = process_data.status;
		} else {
			self.processes.push(process_data);
		}
	}
}
