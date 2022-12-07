use async_trait::async_trait;
// use topos_commands::Command;
//
// use crate::errors::StorageError;

pub trait Command {
    type Result: 'static;
}

#[async_trait]
pub trait CommandHandler<C: Command> {
    type Error;

    /// This method is used by the Connection to handle a command and mutate its state if needed
    ///
    /// # Errors
    ///
    /// This function will return an error if a network or state inconsistency is detected
    async fn handle(&mut self, command: C) -> Result<C::Result, Self::Error>;
}

#[macro_export]
macro_rules! RegisterCommands {

    ($enum_name:ident, $error:ident, $($command:ident),+) => {
        #[derive(Debug)]
        pub enum $enum_name {
            $(
                $command(
                    $command,
                    oneshot::Sender<Result<<$command as Command>::Result, $error>>,
                ),
            )*
        }


        $(
            impl $command {
                #[allow(dead_code)]
                pub(crate) async fn send_to(self, tx: &mpsc::Sender<$enum_name>) -> Result<<Self as Command>::Result, $error> {
                    let (response_channel, receiver) = oneshot::channel();

                    tx.send($enum_name::$command(self, response_channel)).await?;

                    receiver.await?
                }


            }
        )*
    };

    ($enum_name:ident, $handler:ty, $($command:ident),+) => {
        RegisterCommands!($enum_name, (), $($command)*);
    };
}
