use std::sync::Arc;

use ::tokio::sync::mpsc::error::SendError;
use ::tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use ::tokio::task::JoinHandle;
use async_trait::async_trait;
pub use troupe_macro::actor;

pub use crate::tokio::TokioUnbounded;

mod test;

#[non_exhaustive]
pub struct ActorSpawn<T> {
	pub actor:       Arc<T>,
	pub join_handle: JoinHandle<Result<(), ()>>,
}

#[async_trait]
pub trait RoleSender<T: Send>: Sync {
	type Error;
	async fn send(&self, msg: T) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait RoleReceiver<T: Send> {
	async fn recv(&mut self) -> Option<T>;
}

mod tokio {
	use super::*;

	#[async_trait]
	impl<T: Send> RoleSender<T> for UnboundedSender<T> {
		type Error = SendError<T>;

		async fn send(&self, msg: T) -> Result<(), SendError<T>> {
			self.send(msg)
		}
	}

	#[async_trait]
	impl<T: Send> RoleReceiver<T> for UnboundedReceiver<T> {
		async fn recv(&mut self) -> Option<T> {
			self.recv().await
		}
	}

	pub struct TokioUnbounded<T>(std::marker::PhantomData<T>);
	impl<T: Send> super::Channel for TokioUnbounded<T> {
		type Input = ();
		type Item = T;
		type Receiver = UnboundedReceiver<T>;
		type Sender = UnboundedSender<T>;

		fn new(_: ()) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
			unbounded_channel()
		}
	}
}

pub trait Channel {
	type Input;
	type Item: Send + Sized;
	type Sender: RoleSender<Self::Item>;
	type Receiver: RoleReceiver<Self::Item>;
	fn new(data: Self::Input) -> (Self::Sender, Self::Receiver);
	fn new_default() -> (Self::Sender, Self::Receiver)
	where
		Self::Input: Default,
	{
		Self::new(Self::Input::default())
	}
}

pub trait Role {
	type Payload: Sized + Send;
	type Channel: Channel<Item = Self::Payload>;
}
