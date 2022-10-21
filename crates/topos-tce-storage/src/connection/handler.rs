use async_trait::async_trait;

use crate::{command::Command, errors::StorageError};

#[async_trait]
pub(crate) trait CommandHandler<C: Command> {
    /// This method is used by the Connection to handle a command and mutate its state if needed
    ///
    /// # Errors
    ///
    /// This function will return an error if a network or state inconsistency is detected
    async fn handle(&mut self, command: C) -> Result<C::Result, StorageError>;
}
