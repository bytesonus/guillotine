
use futures::{future, Future};
use future::Either;
use crate::utils::logger;

#[async_trait]
pub trait Exitable<O> {
	async fn exitable(self) -> O;
}

#[async_trait]
impl<F, O: 'static> Exitable<O> for F
where
	F: Future<Output = O> + Send,
{
	async fn exitable(self) -> O {
		let mut close_receiver = crate::CLOSE_RECEIVER.lock().await;
		let receiver = close_receiver.as_mut().unwrap();
		match future::select(Box::pin(self), receiver).await {
			Either::Left((result, _)) => result,
			Either::Right((..)) => {
				logger::warn("Recieved exit code. Quitting...");
				std::process::exit(0);
			},
		}
	}
}