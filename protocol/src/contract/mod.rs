use bitcoin_hashes::{hash160, Hash};
use bitcoincash::blockdata::{opcodes, script::Builder};

use crate::keys::bitcoin::{address, PublicKey};

const CONTRACT_BYTECODE: [u8; 47] = hex_literal::hex!("c3519dc4519d00c600cc949d00cb009c6300cd7888547978a85379bb675279b27500cd54798854790088686d6d7551");

#[derive(Debug, Clone)]
pub struct Contract {
    pub mining_fee: i64,
    pub success_output: Vec<u8>,
    pub pubkey_ves: PublicKey,
    pub timelock: i64,
    pub failed_output: Vec<u8>,
}

impl Contract {
    pub fn script(&self) -> Vec<u8> {
        let pubkey = bitcoincash::PublicKey::from_slice(&self.pubkey_ves.to_bytes()).unwrap();
        let mut contract = Builder::new()
            .push_slice(&self.failed_output)
            .push_int(self.timelock)
            .push_key(&pubkey)
            .push_slice(&self.success_output)
            .push_int(self.mining_fee)
            .into_script()
            .to_bytes();

        contract.extend_from_slice(&CONTRACT_BYTECODE);
        contract
    }

    pub fn locking_script(&self) -> Vec<u8> {
        let hash = hash160::Hash::hash(&self.script());
        Builder::new()
            .push_opcode(opcodes::all::OP_HASH160)
            .push_slice(&hash.to_byte_array())
            .push_opcode(opcodes::all::OP_EQUAL)
            .into_script()
            .to_bytes()
    }

    pub fn unlocking_script(&self, unlocker: &[u8]) -> Vec<u8> {
        let locking = self.script();

        Builder::new()
            .push_slice(unlocker)
            .push_slice(&locking)
            .into_script()
            .to_bytes()
    }

    pub fn cash_address(&self) -> String {
        let hash = hash160::Hash::hash(&self.script());
        address::encode(&hash.to_byte_array(), "bitcoincash", 8)
    }

    pub fn cash_token_address(&self) -> String {
        let hash = hash160::Hash::hash(&self.script());
        address::encode(&hash.to_byte_array(), "bitcoincash", 24)
    }
}

#[derive(Debug, Clone)]
pub struct ContractPair {
    pub swaplock: Contract,
    pub refund: Contract,
}

impl ContractPair {
    pub fn create(
        mining_fee: i64,
        bob_receiving: Vec<u8>,
        bob_pubkey_ves: PublicKey,
        alice_receiving: Vec<u8>,
        alice_pubkey_ves: PublicKey,
    ) -> ContractPair {
        let refund = Contract {
            mining_fee,
            success_output: bob_receiving,
            pubkey_ves: alice_pubkey_ves,
            timelock: 0,
            failed_output: alice_receiving.clone(),
        };

        let swaplock = Contract {
            mining_fee,
            success_output: alice_receiving,
            pubkey_ves: bob_pubkey_ves,
            timelock: 0,
            failed_output: refund.locking_script(),
        };

        ContractPair { swaplock, refund }
    }
}

#[cfg(test)]
mod test {
    use crate::keys::bitcoin::PublicKey;

    use crate::contract::Contract;

    #[test]
    fn should_have_correct_address() {
        let pubkey_ves = PublicKey::from_str(
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
        };

        assert!(refund.cash_address() == "bitcoincash:prmnwxmmaq58h22jt7qrjmutnkrmrfm4j57zy4cf45");
        assert!(
            refund.cash_token_address() == "bitcoincash:rrmnwxmmaq58h22jt7qrjmutnkrmrfm4j5eghtk028"
        );
    }
}
