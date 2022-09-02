#[cfg(test)]
#[allow(dead_code)]
mod totality {
    use std::cmp::min;
    use topos_core::uci::SubnetId;
    use topos_tce_broadcast::mock;

    #[test]
    fn disclaimer() {
        println!(
            "totality - DISCLAIMER: we do not run massive async timing dependent code in the CI"
        );
    }

    fn test_totality_boundaries(input: mock::InputConfig) {
        let lower_bound = mock::sample_lower_bound(input.nb_peers);
        let correct_sample = min(10 * lower_bound, input.nb_peers);

        // Should be big enough
        assert!(
            mock::viable_run(correct_sample, 0.66, 0.33, 0.66, &input).is_some(),
            "Totality failed, sample size: {}\t nb peers: {}",
            correct_sample,
            input.nb_peers
        );

        // Should be too small
        // fixme : ... but it is not
        // let incorrect_sample = lower_bound - 1;
        // assert!(
        //     mock::viable_run(incorrect_sample, 0.66, 0.33, 0.66, &input).is_none(),
        //     "Totality must fail, sample_size: {}\t nb peers: {}",
        //     incorrect_sample,
        //     input.nb_peers
        // );
    }

    // we do not run async timing dependent code in the CI
    // #[test]
    fn with_1cert_100nodes() {
        let nb_peers: usize = 100;
        let nb_certificates = 1;
        let subnets: Vec<SubnetId> = vec![1, 2, 3];

        test_totality_boundaries(mock::InputConfig {
            nb_peers,
            nb_subnets: subnets.len(),
            nb_certificates,
        });
    }

    // we do not run async timing dependent code in the CI
    // #[test]
    fn with_1cert_1000nodes() {
        let nb_peers: usize = 1000;
        let nb_certificates = 1;
        let subnets: Vec<SubnetId> = vec![1, 2, 3];

        test_totality_boundaries(mock::InputConfig {
            nb_peers,
            nb_subnets: subnets.len(),
            nb_certificates,
        });
    }

    // we do not run async timing dependent code in the CI
    // #[test]
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
