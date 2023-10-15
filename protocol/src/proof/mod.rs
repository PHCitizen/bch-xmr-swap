use ::conquer_once::Lazy;
use sha2::Sha256;
use sigma_fun::{
    ed25519::{
        curve25519_dalek,
        curve25519_dalek::{edwards::CompressedEdwardsY, scalar::Scalar as ScalarDalek},
    },
    ext::dl_secp256k1_ed25519_eq::{CrossCurveDLEQ, CrossCurveDLEQProof},
    secp256k1::fun::Point as PointP,
    HashTranscript,
};

use crate::keys::bitcoin;

pub static CROSS_CURVE_PROOF_SYSTEM: Lazy<
    CrossCurveDLEQ<HashTranscript<Sha256, rand_chacha::ChaCha20Rng>>,
> = Lazy::new(|| {
    CrossCurveDLEQ::<HashTranscript<Sha256, rand_chacha::ChaCha20Rng>>::new(
        sigma_fun::secp256k1::fun::G.normalize().normalize(),
        curve25519_dalek::constants::ED25519_BASEPOINT_POINT,
    )
});

pub fn prove(
    privkey: &monero::PrivateKey,
) -> (CrossCurveDLEQProof, (bitcoin::PublicKey, monero::PublicKey)) {
    let mut rng = rand::thread_rng();
    let scalar = ScalarDalek::from_bytes_mod_order(privkey.to_bytes());
    let (proof, (point, ed_point)) =
        CrossCurveDLEQ::prove(&CROSS_CURVE_PROOF_SYSTEM, &scalar, &mut rng);

    (
        proof,
        (
            bitcoin::PublicKey::from_point(point),
            monero::PublicKey::from_slice(ed_point.compress().as_bytes()).unwrap(),
        ),
    )
}

pub fn verify(
    proof: &CrossCurveDLEQProof,
    bch: crate::keys::bitcoin::PublicKey,
    xmr_pubkey: monero::PublicKey,
) -> bool {
    let point = PointP::from_bytes(bch.to_bytes()).unwrap();
    let edward_point = CompressedEdwardsY::from_slice(xmr_pubkey.as_bytes())
        .decompress()
        .unwrap();

    CrossCurveDLEQ::verify(&CROSS_CURVE_PROOF_SYSTEM, proof, (point, edward_point))
}
