use async_trait::async_trait;

pub trait Command {
    type Result: 'static;
}

#[async_trait]
pub trait CommandHandler<C: Command> {
    type Error;

    /// This method is used by a handler in order to execute a command and mutate its state if needed
    ///
    /// # Errors
    ///
    /// This function will return an error if the command can't be executed'
    async fn handle(&mut self, command: C) -> Result<C::Result, Self::Error>;
}

#[macro_export]
macro_rules! RegisterCommands {

    (name = $enum_name:ident, error = $error:ident, commands = [$($command:ident),+]) => {
        pub enum $enum_name {
            $(
                $command(
                    $command,
                    oneshot::Sender<Result<<$command as Command>::Result, $error>>,
                ),
            )*
        }

        impl std::fmt::Debug for $enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                #[allow(non_snake_case)]
                match self {
                    $(
                        Self::$command(
                            $command,
                            _,
                        ) => std::fmt::Debug::fmt(&$command, f),
                    )*
                }
            }

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

    (name = $enum_name:ident, commands = [$($command:ident),+]) => {
        RegisterCommands!(name = $enum_name, error = (), commands = [$($command)*]);
    };
}
