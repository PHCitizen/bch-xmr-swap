use ::conquer_once::Lazy;
use sha2::Sha256;
use sigma_fun::{
    ed25519::{
        curve25519_dalek,
        curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar as ScalarDalek},
    },
    ext::dl_secp256k1_ed25519_eq::{CrossCurveDLEQ, CrossCurveDLEQProof},
    secp256k1::fun::Point as PointP,
    HashTranscript,
};

pub static CROSS_CURVE_PROOF_SYSTEM: Lazy<
    CrossCurveDLEQ<HashTranscript<Sha256, rand_chacha::ChaCha20Rng>>,
> = Lazy::new(|| {
    CrossCurveDLEQ::<HashTranscript<Sha256, rand_chacha::ChaCha20Rng>>::new(
        sigma_fun::secp256k1::fun::G.normalize().normalize(),
        curve25519_dalek::constants::ED25519_BASEPOINT_POINT,
    )
});

pub fn prove(scalar: &ScalarDalek) -> (CrossCurveDLEQProof, (PointP, EdwardsPoint)) {
    let mut rng = rand::thread_rng();
    CrossCurveDLEQ::prove(&CROSS_CURVE_PROOF_SYSTEM, scalar, &mut rng)
}

pub fn verify(proof: &CrossCurveDLEQProof, claim: (PointP, EdwardsPoint)) -> bool {
    CrossCurveDLEQ::verify(&CROSS_CURVE_PROOF_SYSTEM, proof, claim)
}
