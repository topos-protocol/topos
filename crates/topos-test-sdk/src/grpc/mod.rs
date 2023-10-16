pub mod behaviour {
    #[rustfmt::skip]
    pub mod helloworld;

    #[rustfmt::skip]
    pub mod noop;
}

pub mod implementations {
    use std::time::Duration;

    use async_trait::async_trait;
    use tonic::{Request, Response, Status};

    use super::behaviour::{
        helloworld::{greeter_server::Greeter, HelloReply, HelloRequest, HelloWithDelayRequest},
        noop::{noop_server::Noop, NoopRequest, NoopResponse},
    };

    #[derive(Default)]
    pub struct DummyServer {}

    #[async_trait]
    impl Greeter for DummyServer {
        async fn say_hello(
            &self,
            request: Request<HelloRequest>,
        ) -> Result<Response<HelloReply>, Status> {
            Ok(Response::new(HelloReply {
                message: format!("Hello {}", request.into_inner().name),
            }))
        }

        async fn say_hello_with_delay(
            &self,
            request: Request<HelloWithDelayRequest>,
        ) -> Result<Response<HelloReply>, Status> {
            let request = request.into_inner();
            tokio::time::sleep(Duration::from_secs(request.delay_in_seconds)).await;

            Ok(Response::new(HelloReply {
                message: format!("Hello {}", request.name),
            }))
        }
    }

    #[derive(Default)]
    pub struct NoopServer {}

    #[async_trait]
    impl Noop for NoopServer {
        async fn do_nothing(
            &self,
            _: Request<NoopRequest>,
        ) -> Result<Response<NoopResponse>, Status> {
            Ok(Response::new(NoopResponse {}))
        }
    }
}
