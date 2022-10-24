use std::collections::HashMap;
use topos_core::uci::{Address, Amount, CrossChainTransaction};
use topos_tce_broadcast::uci::{Certificate, CertificateId, CrossChainTransactionData, SubnetId};

/// Generate and assign nb_cert number of certificates to existing subnets
/// Could be different number of certificates per subnet
pub fn generate_cert(
    subnets: &Vec<SubnetId>,
    number_of_certificates_per_subnet: usize,
) -> HashMap<SubnetId, Vec<Certificate>> {
    let mut nonce_state: HashMap<SubnetId, CertificateId> = HashMap::new();
    let mut result: HashMap<SubnetId, Vec<Certificate>> = HashMap::new();
    for subnet in subnets {
        result.insert(subnet.clone(), Vec::new());
    }

    // Initialize the genesis of all subnets
    for subnet in subnets {
        nonce_state.insert(subnet.to_string(), 0.to_string());
    }

    let mut gen_cert = |selected_subnet: String| -> Certificate {
        let last_cert_id = nonce_state.get_mut(&selected_subnet).unwrap();
        // Add one cross chain transaction for every other subnet
        let terminal_subnets = subnets
            .iter()
            .filter(|sub| *sub != &selected_subnet)
            .cloned()
            .collect::<Vec<_>>();
        let mut cross_chain_transactions = Vec::new();
        for (index, terminal_subnet_id) in terminal_subnets.into_iter().enumerate() {
            cross_chain_transactions.push(CrossChainTransaction {
                terminal_subnet_id: terminal_subnet_id.clone(),
                transaction_data: CrossChainTransactionData::AssetTransfer {
                    asset_id: "TST_SUBNET_".to_string() + &terminal_subnet_id,
                    amount: Amount::from((index + 1) * 100),
                },
                recipient_addr: Address::from("0x0000000000000000000000000000000000000002"),
                sender_addr: Address::from("0x0000000000000000000000000000000000000001"),
            })
        }

        let gen_cert = Certificate::new(
            last_cert_id.clone(),
            selected_subnet,
            cross_chain_transactions,
        );
        *last_cert_id = gen_cert.cert_id.clone();
        gen_cert
    };

    for selected_subnet in subnets {
        for _ in 0..number_of_certificates_per_subnet {
            let cert = gen_cert(selected_subnet.clone());
            result
                .entry(selected_subnet.clone())
                .or_insert(Vec::new())
                .push(cert);
        }
    }
    result
}
