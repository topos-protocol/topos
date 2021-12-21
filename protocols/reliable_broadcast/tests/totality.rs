#[cfg(test)]
mod totality {
    use std::cmp::min;
    use tce_uci::SubnetId;
    use topos_tce_protocols_reliable_broadcast::mock;

    fn test_totality_boundaries(input: mock::InputConfig) {
        let lower_bound = mock::sample_lower_bound(input.nb_peers);
        let correct_sample = min(10 * lower_bound, input.nb_peers);
        let incorrect_sample = lower_bound - 1;

        // Should be big enough
        assert!(
            mock::viable_run(correct_sample, &input).is_some(),
            "Totality failed, sample size: {}\t nb peers: {}",
            correct_sample,
            input.nb_peers
        );

        // Should be too small
        assert!(
            mock::viable_run(incorrect_sample, &input).is_none(),
            "Totality must fail, sample_size: {}\t nb peers: {}",
            incorrect_sample,
            input.nb_peers
        );
    }

    #[test]
    fn with_1cert_100nodes() {
        let nb_peers: usize = 100;
        let nb_certificates = 1;
        let subnets: Vec<SubnetId> = vec![0, 1, 2];

        test_totality_boundaries(mock::InputConfig {
            nb_peers,
            nb_subnets: subnets.len(),
            nb_certificates,
        });
    }

    #[test]
    fn with_1cert_1000nodes() {
        let nb_peers: usize = 1000;
        let nb_certificates = 1;
        let subnets: Vec<SubnetId> = vec![0, 1, 2];

        test_totality_boundaries(mock::InputConfig {
            nb_peers,
            nb_subnets: subnets.len(),
            nb_certificates,
        });
    }

    #[test]
    fn with_10cert_100nodes() {
        let nb_peers: usize = 100;
        let nb_certificates = 10;
        let subnets: Vec<SubnetId> = vec![0, 1, 2];

        test_totality_boundaries(mock::InputConfig {
            nb_peers,
            nb_subnets: subnets.len(),
            nb_certificates,
        });
    }
}
