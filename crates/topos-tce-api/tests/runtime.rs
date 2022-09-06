use test_log::test;
use tokio::spawn;
use topos_tce_api::Runtime;

#[test(tokio::test)]
async fn runtime_can_dispatch_a_cert() {
    let (_runtime_client, launcher) = Runtime::builder().build();

    spawn(launcher.launch());
}
