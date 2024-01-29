use crate::p2p;

#[test]
fn increment_echo_failure_ser() {
    let m = &p2p::P2P_MESSAGE_SERIALIZE_FAILURE_TOTAL;

    m.with_label_values(&["echo"]).inc();

    assert_eq!(m.get_metric_with_label_values(&["echo"]).unwrap().get(), 1);
    assert_eq!(m.get_metric_with_label_values(&["ready"]).unwrap().get(), 0);
}

#[test]
fn increment_echo_failure_des() {
    let m = &p2p::P2P_MESSAGE_DESERIALIZE_FAILURE_TOTAL;

    m.with_label_values(&["echo"]).inc();

    assert_eq!(m.get_metric_with_label_values(&["echo"]).unwrap().get(), 1);
    assert_eq!(m.get_metric_with_label_values(&["ready"]).unwrap().get(), 0);
}
