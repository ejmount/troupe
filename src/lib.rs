use async_trait::async_trait;
mod test;
#[async_trait]
trait RoleSender: Sized {
    type Item;
    type Error;
    async fn send(&self, msg: impl Into<Self::Item> + Send) -> Result<(), Self::Error>;
}

mod tokio {
    use super::async_trait;
    use crate::RoleSender;

    #[async_trait]
    impl<T: Send> RoleSender for tokio::sync::mpsc::UnboundedSender<T> {
        type Item = T;
        type Error = tokio::sync::mpsc::error::SendError<T>;
        async fn send(&self, msg: impl Into<T> + Send) -> Result<(), <Self as RoleSender>::Error> {
            self.send(msg.into())
        }
    }
}

#[async_trait]
trait RoleInfo {
    type Payload: Sized + Send;
    type Sender: RoleSender<Item = Self::Payload>;
    type Receiver: Sized;
}
