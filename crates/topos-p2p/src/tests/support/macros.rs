#[macro_export]
macro_rules! wait_for_event {
    ($node:ident, matches: $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {
        let assertion = async {
            while let Some(event) = $node.next().await {
                println!("Event: {:?}", event);
                if matches!(event, $( $pattern )|+ $( if $guard )?) {
                    break;
                }
            }
        };

        if let Err(_) = tokio::time::timeout(std::time::Duration::from_millis(100), assertion).await
        {
            panic!("Timeout waiting for event");
        }
    };
}
