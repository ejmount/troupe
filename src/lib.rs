use async_trait::async_trait;
mod test;
#[async_trait]
trait RoleSender: Sized {
    type Item;
    type Error;
    async fn send(&self, msg: impl Into<Self::Item> + Send) -> Result<(), Self::Error>;
}

mod tokio {
    use tokio::sync::mpsc::error::SendError;
    use tokio::sync::mpsc::UnboundedSender;

    use super::async_trait;
    use crate::RoleSender;

    #[async_trait]
    impl<T: Send> RoleSender for UnboundedSender<T> {
        type Item = T;
        type Error = SendError<T>;
        async fn send(&self, msg: impl Into<T> + Send) -> Result<(), SendError<T>> {
            let val = msg.into();
            self.send(val)
        }
    }
}

pub trait Role {
	type Payload: Sized + Send;
	type Sender: RoleSender<Item = Self::Payload>;
	type Receiver: Sized;
}
