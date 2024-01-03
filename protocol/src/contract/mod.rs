use bitcoin_hashes::{hash160, Hash};
use bitcoincash::{
    blockdata::{opcodes, script::Builder},
    Transaction,
};
use serde::{Deserialize, Serialize};

use crate::keys::bitcoin::{address, Network};

const CONTRACT_BYTECODE: [u8; 47] = hex_literal::hex!("c3519dc4519d00c600cc949d00cb009c6300cd7888547978a85379bb675279b27500cd54798854790088686d6d7551");

#[derive(Debug)]
pub enum TransactionType {
    ToSwapLock,
    ToRefund,
    ToBob,
    ToAlice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub mining_fee: u64,
    #[serde(with = "hex")]
    pub success_output: Vec<u8>,
    pub pubkey_ves: bitcoincash::PublicKey,
    pub timelock: i64,
    #[serde(with = "hex")]
    pub failed_output: Vec<u8>,

    pub bch_network: Network,
}

impl Contract {
    pub fn script(&self) -> Vec<u8> {
        let mut contract = Builder::new()
            .push_slice(&self.failed_output)
            .push_int(self.timelock)
            .push_key(&self.pubkey_ves)
            .push_slice(&self.success_output)
            .push_int(self.mining_fee as i64)
            .into_script()
            .to_bytes();

        contract.extend_from_slice(&CONTRACT_BYTECODE);
        contract
    }

    #[inline]
    pub fn script_hash(&self) -> [u8; 20] {
        hash160::Hash::hash(&self.script()).to_byte_array()
    }

    // ? Idk returning raw script becomes error for caller,
    // ? we need to convert to bytes, then caller need to convert back to script
    pub fn locking_script(&self) -> Vec<u8> {
        let hash = self.script_hash();
        Builder::new()
            .push_opcode(opcodes::all::OP_HASH160)
            .push_slice(&hash)
            .push_opcode(opcodes::all::OP_EQUAL)
            .into_script()
            .to_bytes()
    }

    // ? Idk returning raw script becomes error for caller,
    // ?we need to convert to bytes, then caller need to convert back to script
    pub fn unlocking_script(&self, unlocker: &[u8]) -> Vec<u8> {
        let locking = self.script();

        Builder::new()
            .push_slice(unlocker)
            .push_slice(&locking)
            .into_script()
            .to_bytes()
    }

    pub fn cash_address(&self) -> String {
        let hash = self.script_hash();
        match self.bch_network {
            Network::Mainnet => address::encode(&hash, "bitcoincash", 8),
            Network::Testnet => address::encode(&hash, "bchtest", 8),
            Network::Regtest => address::encode(&hash, "bchreg", 8),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractPair {
    pub swaplock: Contract,
    pub refund: Contract,
    alice_receiving: Vec<u8>,
    bob_receiving: Vec<u8>,
    swaplock_in_sats: u64,
}

impl ContractPair {
    pub fn create(
        mining_fee: u64,
        bob_receiving: Vec<u8>,
        bob_pubkey_ves: bitcoincash::PublicKey,
        alice_receiving: Vec<u8>,
        alice_pubkey_ves: bitcoincash::PublicKey,
        timelock1: i64,
        timelock2: i64,
        bch_network: Network,
        swaplock_in: bitcoincash::Amount,
    ) -> ContractPair {
        let refund = Contract {
            mining_fee,
            success_output: bob_receiving.clone(),
            pubkey_ves: alice_pubkey_ves,
            timelock: timelock2,
            failed_output: alice_receiving.clone(),
            bch_network,
        };

        let swaplock = Contract {
            mining_fee,
            success_output: alice_receiving.clone(),
            pubkey_ves: bob_pubkey_ves,
            timelock: timelock1,
            failed_output: refund.locking_script(),
            bch_network,
        };

        ContractPair {
            swaplock,
            refund,
            alice_receiving,
            bob_receiving,
            swaplock_in_sats: swaplock_in.to_sat(),
        }
    }

    pub fn analyze_tx(
        &self,
        transaction: Transaction,
    ) -> Option<(bitcoincash::OutPoint, TransactionType)> {
        let swaplock = self.swaplock.locking_script();
        let refund = self.refund.locking_script();

        if transaction.input.len() == 1 && transaction.output.len() == 1 {
            let input = transaction.input[0].script_sig.to_p2sh().to_bytes();
            let output = transaction.output[0].script_pubkey.to_bytes();

            if input == swaplock || input == refund {
                let tx_type;
                if output == self.alice_receiving {
                    tx_type = TransactionType::ToAlice;
                } else if output == self.bob_receiving {
                    tx_type = TransactionType::ToBob;
                // } else if output == refund {
                } else {
                    tx_type = TransactionType::ToRefund;
                }

                return Some((bitcoincash::OutPoint::new(transaction.txid(), 0), tx_type));
            }
        }

        for (vout, out) in transaction.output.iter().enumerate() {
            if out.script_pubkey.to_bytes() == swaplock && out.value == self.swaplock_in_sats {
                return Some((
                    bitcoincash::OutPoint::new(transaction.txid(), vout as u32),
                    TransactionType::ToSwapLock,
                ));
            }
        }

        return None;
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::contract::Contract;

    #[test]
    fn should_have_correct_address() {
        let pubkey_ves = bitcoincash::PublicKey::from_str(
            "02ee2cbe75e3d2a9b5049ac73122c229627a49bd289f71e05075b2c60090766128",
        )
        .unwrap();
        let output = hex::decode("76a91447fe8a0ca161ebc0090c9d46f81582c579c594a788ac").unwrap();

        let refund = Contract {
            mining_fee: 1000,
            success_output: output.clone(),
            pubkey_ves,
            timelock: 1000,
            failed_output: output,
            bch_network: crate::keys::bitcoin::Network::Testnet,
        };

        assert_eq!(
            refund.cash_address(),
            "bitcoincash:prmnwxmmaq58h22jt7qrjmutnkrmrfm4j57zy4cf45"
        );
    }
}
