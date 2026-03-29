use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // X25519 key generation: returns [pub_bytes(32), priv_bytes(32)]
    g.set(
        "__braille_crypto_x25519_generate",
        Function::new(ctx.clone(), || -> Vec<Vec<u8>> {
            use x25519_dalek::{StaticSecret, PublicKey};
            let secret = StaticSecret::random_from_rng(rand_core::OsRng);
            let public = PublicKey::from(&secret);
            vec![public.as_bytes().to_vec(), secret.to_bytes().to_vec()]
        })
        .unwrap(),
    )
    .unwrap();

    // X25519 ECDH: derive_bits(priv_bytes, pub_bytes) -> shared_secret or empty on error
    g.set(
        "__braille_crypto_x25519_derive_bits",
        Function::new(
            ctx.clone(),
            |priv_bytes: Vec<u8>, pub_bytes: Vec<u8>| -> Vec<u8> {
                use x25519_dalek::{StaticSecret, PublicKey};
                let mut priv_arr = [0u8; 32];
                priv_arr.copy_from_slice(&priv_bytes);
                let secret = StaticSecret::from(priv_arr);

                let mut pub_arr = [0u8; 32];
                pub_arr.copy_from_slice(&pub_bytes);
                let their_public = PublicKey::from(pub_arr);

                let shared = secret.diffie_hellman(&their_public);

                // Return empty vec for all-zero result (small-order points)
                // JS layer converts this to an OperationError rejection
                if shared.as_bytes().iter().all(|&b| b == 0) {
                    return vec![];
                }

                shared.as_bytes().to_vec()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // Ed25519 key generation: returns [pub_bytes(32), priv_bytes(32)]
    g.set(
        "__braille_crypto_ed25519_generate",
        Function::new(ctx.clone(), || -> Vec<Vec<u8>> {
            use ed25519_dalek::SigningKey;
            let signing_key = SigningKey::generate(&mut rand_core::OsRng);
            let verifying_key = signing_key.verifying_key();
            vec![
                verifying_key.as_bytes().to_vec(),
                signing_key.to_bytes().to_vec(),
            ]
        })
        .unwrap(),
    )
    .unwrap();

    // Ed25519 sign: (priv_bytes, data) -> signature(64)
    g.set(
        "__braille_crypto_ed25519_sign",
        Function::new(
            ctx.clone(),
            |priv_bytes: Vec<u8>, data: Vec<u8>| -> Vec<u8> {
                use ed25519_dalek::SigningKey;
                use ed25519_dalek::Signer;
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(&priv_bytes);
                let signing_key = SigningKey::from_bytes(&key_arr);
                let sig = signing_key.sign(&data);
                sig.to_bytes().to_vec()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // Ed25519 verify: (pub_bytes, signature, data) -> bool
    g.set(
        "__braille_crypto_ed25519_verify",
        Function::new(
            ctx.clone(),
            |pub_bytes: Vec<u8>, signature: Vec<u8>, data: Vec<u8>| -> bool {
                use ed25519_dalek::{Signature, VerifyingKey};
                use ed25519_dalek::Verifier;
                let mut key_arr = [0u8; 32];
                key_arr.copy_from_slice(&pub_bytes);
                let verifying_key =
                    VerifyingKey::from_bytes(&key_arr).expect("invalid Ed25519 public key");
                let mut sig_arr = [0u8; 64];
                sig_arr.copy_from_slice(&signature);
                let sig = Signature::from_bytes(&sig_arr);
                verifying_key.verify(&data, &sig).is_ok()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ECDH P-256/P-384 key generation: (curve) -> [pub_uncompressed, priv_bytes]
    g.set(
        "__braille_crypto_ecdh_generate",
        Function::new(ctx.clone(), |curve: String| -> Vec<Vec<u8>> {
            match curve.as_str() {
                "P-256" => {
                    let nz = p256::NonZeroScalar::random(&mut rand_core::OsRng);
                    let public = p256::PublicKey::from_secret_scalar(&nz);
                    let priv_bytes = nz.to_bytes().to_vec();
                    let pub_bytes = public.to_sec1_bytes().to_vec();
                    vec![pub_bytes, priv_bytes]
                }
                "P-384" => {
                    let nz = p384::NonZeroScalar::random(&mut rand_core::OsRng);
                    let public = p384::PublicKey::from_secret_scalar(&nz);
                    let priv_bytes = nz.to_bytes().to_vec();
                    let pub_bytes = public.to_sec1_bytes().to_vec();
                    vec![pub_bytes, priv_bytes]
                }
                other => panic!("NotSupportedError: ECDH curve '{other}' not supported"),
            }
        })
        .unwrap(),
    )
    .unwrap();

    // ECDH derive: (curve, priv_bytes, pub_uncompressed_bytes) -> shared_secret
    g.set(
        "__braille_crypto_ecdh_derive",
        Function::new(
            ctx.clone(),
            |curve: String, priv_bytes: Vec<u8>, pub_bytes: Vec<u8>| -> Vec<u8> {
                match curve.as_str() {
                    "P-256" => {
                        use p256::ecdh::diffie_hellman;
                        use p256::elliptic_curve::sec1::FromEncodedPoint;
                        let scalar = p256::NonZeroScalar::from_repr(
                            *p256::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-256 private key");
                        let encoded = p256::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-256 public key encoding");
                        let public = p256::PublicKey::from_encoded_point(&encoded)
                            .expect("invalid P-256 public key point");
                        let shared = diffie_hellman(scalar, public.as_affine());
                        shared.raw_secret_bytes().to_vec()
                    }
                    "P-384" => {
                        use p384::ecdh::diffie_hellman;
                        use p384::elliptic_curve::sec1::FromEncodedPoint;
                        let scalar = p384::NonZeroScalar::from_repr(
                            *p384::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-384 private key");
                        let encoded = p384::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-384 public key encoding");
                        let public = p384::PublicKey::from_encoded_point(&encoded)
                            .expect("invalid P-384 public key point");
                        let shared = diffie_hellman(scalar, public.as_affine());
                        shared.raw_secret_bytes().to_vec()
                    }
                    other => panic!("NotSupportedError: ECDH curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ECDSA sign: (curve, hash, priv_bytes, data) -> signature (DER)
    g.set(
        "__braille_crypto_ecdsa_sign",
        Function::new(
            ctx.clone(),
            |curve: String, _hash: String, priv_bytes: Vec<u8>, data: Vec<u8>| -> Vec<u8> {
                match curve.as_str() {
                    "P-256" => {
                        use p256::ecdsa::{SigningKey, Signature};
                        use p256::ecdsa::signature::Signer;
                        let signing_key = SigningKey::from_bytes(
                            p256::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-256 signing key");
                        let sig: Signature = signing_key.sign(&data);
                        sig.to_bytes().to_vec()
                    }
                    "P-384" => {
                        use p384::ecdsa::{SigningKey, Signature};
                        use p384::ecdsa::signature::Signer;
                        let signing_key = SigningKey::from_bytes(
                            p384::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-384 signing key");
                        let sig: Signature = signing_key.sign(&data);
                        sig.to_bytes().to_vec()
                    }
                    other => panic!("NotSupportedError: ECDSA curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ECDSA verify: (curve, hash, pub_bytes, signature, data) -> bool
    g.set(
        "__braille_crypto_ecdsa_verify",
        Function::new(
            ctx.clone(),
            |curve: String,
             _hash: String,
             pub_bytes: Vec<u8>,
             signature: Vec<u8>,
             data: Vec<u8>|
             -> bool {
                match curve.as_str() {
                    "P-256" => {
                        use p256::ecdsa::{Signature, VerifyingKey};
                        use p256::ecdsa::signature::Verifier;
                        let encoded = p256::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-256 public key");
                        let vk = VerifyingKey::from_encoded_point(&encoded)
                            .expect("invalid P-256 verifying key");
                        let sig = Signature::from_slice(&signature)
                            .expect("invalid P-256 signature");
                        vk.verify(&data, &sig).is_ok()
                    }
                    "P-384" => {
                        use p384::ecdsa::{Signature, VerifyingKey};
                        use p384::ecdsa::signature::Verifier;
                        let encoded = p384::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-384 public key");
                        let vk = VerifyingKey::from_encoded_point(&encoded)
                            .expect("invalid P-384 verifying key");
                        let sig = Signature::from_slice(&signature)
                            .expect("invalid P-384 signature");
                        vk.verify(&data, &sig).is_ok()
                    }
                    other => panic!("NotSupportedError: ECDSA curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();
}
