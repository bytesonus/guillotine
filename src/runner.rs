use crate::{
	host,
	models::{NodeConfig, RunnerConfig},
	node,
	utils::logger,
};

use async_std::{fs, path::Path};
use futures::{channel::oneshot::channel, future};

pub async fn run(mut config: RunnerConfig) {
	if config.logs.is_some() {
		let log_dir = config.logs.as_ref().unwrap();
		let main_dir = Path::new(log_dir);
		if !main_dir.exists().await {
			fs::create_dir(&main_dir).await.unwrap();
		}
	}

	if config.host.is_some() {
		let host = config.host.as_ref().unwrap();
		if config.name.is_none() {
			config.name = Some(String::from("host"));
		}
		// If the host doesn't have a node, create one
		config.node = Some(NodeConfig {
			connection_type: host.connection_type.clone(),
			ip: Some(String::from("127.0.0.1")),
			port: host.port,
			socket_path: host.socket_path.clone(),
		});

		let (sender, receiver) = channel::<Option<()>>();
		future::join(host::run(config.clone(), sender), async {
			let response = receiver.await;
			if response.is_err() {
				logger::error("Host didn't start properly yet");
				return;
			}

			if response.unwrap().is_none() {
				return;
			}
			node::run(config).await;
		})
		.await;
	} else if config.node.is_some() {
		node::run(config).await;
	}
}

pub async fn on_exit() {
	future::join(host::on_exit(), node::on_exit()).await;
}
