use ecdsa_fun::{
    adaptor::{Adaptor, EncryptedSignature},
    fun::{
        self,
        marker::{NonZero, Secret},
        Point,
    },
    nonce::Deterministic,
    Signature,
};
use sha2::Sha256;
use sigma_fun::ed25519::curve25519_dalek::scalar::Scalar as ScalarDalek;
use sigma_fun::HashTranscript;

type Transcript = HashTranscript<Sha256, rand_chacha::ChaCha20Rng>;
type NonceGen = Deterministic<Sha256>;

pub struct AdaptorSignature;

impl AdaptorSignature {
    pub fn verify(signer: bitcoincash::PublicKey, message: &[u8; 32], sig: &Signature) -> bool {
        let ecdsa: ecdsa_fun::ECDSA<Deterministic<Sha256>> = ecdsa_fun::ECDSA::default();

        let s_monero_bch = Point::from_bytes(signer.inner.serialize()).unwrap();
        ecdsa.verify(&s_monero_bch, &message, &sig)
    }

    pub fn encrypted_sign(
        signer: &bitcoincash::PrivateKey,
        encryption_key: &bitcoincash::PublicKey,
        message: &[u8; 32],
    ) -> EncryptedSignature {
        let adaptor: Adaptor<Transcript, NonceGen> = Adaptor::default();
        let signer = ecdsa_fun::fun::Scalar::from_bytes(signer.inner.secret_bytes())
            .expect("failed to convert PrivateKey -> Scalar")
            .non_zero()
            .expect("failed to convert PrivateKey -> Scalar. non-zero");

        let encryption_key = fun::Point::from_bytes(encryption_key.inner.serialize())
            .expect("failed to convert PublicKey -> Point");

        adaptor.encrypted_sign(&signer, &encryption_key, &message)
    }

    pub fn decrypt_signature(
        decryption_key: &monero::PrivateKey,
        encrypted_sig: EncryptedSignature,
    ) -> Signature {
        let adaptor: Adaptor<Transcript, NonceGen> = Adaptor::default();

        let decryption_key = ScalarDalek::from_bytes_mod_order(decryption_key.to_bytes());
        let decryption_key = scalardalek_to_scalarfun(&decryption_key);

        adaptor.decrypt_signature(&decryption_key, encrypted_sig)
    }

    pub fn recover_decryption_key(
        pubkey: bitcoincash::PublicKey,
        sig: Signature,
        enc_sig: EncryptedSignature,
    ) -> monero::PrivateKey {
        let adaptor: Adaptor<Transcript, NonceGen> = Adaptor::default();
        let pubkey: Point = fun::Point::from_bytes(pubkey.inner.serialize())
            .expect("failed to convert PublicKey -> Point");

        let key_reversed = adaptor
            .recover_decryption_key(&pubkey, &sig, &enc_sig)
            .unwrap();

        let mut big_edian: [u8; 32] = key_reversed.to_bytes();
        big_edian.reverse();
        let little_edian = big_edian;
        monero::PrivateKey::from_slice(&little_edian).unwrap()
    }
}

fn scalardalek_to_scalarfun(scalar: &ScalarDalek) -> ecdsa_fun::fun::Scalar<Secret, NonZero> {
    let mut little_endian_bytes = scalar.to_bytes();

    little_endian_bytes.reverse();
    let big_endian_bytes = little_endian_bytes;

    ecdsa_fun::fun::Scalar::from_bytes(big_endian_bytes)
        .expect("valid scalar")
        .non_zero()
        .expect("non-zero scalar")
}

#[cfg(test)]
mod test {
    use super::AdaptorSignature;
    use crate::keys;

    #[test]
    fn test() {
        let bob = keys::KeyPrivate::random(keys::bitcoin::Network::Testnet);
        let bobpub = keys::KeyPublic::from(bob.clone());
        let alice = keys::KeyPrivate::random(keys::bitcoin::Network::Testnet);
        let alicepub = keys::KeyPublic::from(alice.clone());
        let message = [0u8; 32];

        // bob signed alice output
        let enc_sig = AdaptorSignature::encrypted_sign(&bob.ves, &alicepub.spend_bch, &message);
        dbg!(&enc_sig);

        // alice decrypt the enc_sig
        let dec_sig = AdaptorSignature::decrypt_signature(&alice.monero_spend, enc_sig.clone());
        dbg!(&dec_sig);

        // alice check if dec_sig can unlock swaplock
        let valid = AdaptorSignature::verify(bobpub.ves.clone(), &message, &dec_sig.clone());
        assert!(valid);

        // bob get the decsig on bch tx, and recover alice priv_spend
        let alice_spend_recovered =
            AdaptorSignature::recover_decryption_key(alicepub.spend_bch, dec_sig, enc_sig);

        assert_eq!(
            alice_spend_recovered.to_string(),
            alice.monero_spend.to_string()
        )
    }
}
