use rand_core::RngCore;
use rquickjs::{Ctx, Function};
use p256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};

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

    // X448 key generation: returns [pub_bytes(56), priv_bytes(56)]
    g.set(
        "__braille_crypto_x448_generate",
        Function::new(ctx.clone(), || -> Vec<Vec<u8>> {
            let mut priv_bytes = [0u8; 56];
            rand_core::OsRng.fill_bytes(&mut priv_bytes);
            // X448 base point (u=5)
            let mut base_point = [0u8; 56];
            base_point[0] = 5;
            let pub_bytes = x448::x448(priv_bytes, base_point)
                .expect("X448 key generation failed");
            vec![pub_bytes.to_vec(), priv_bytes.to_vec()]
        })
        .unwrap(),
    )
    .unwrap();

    // X448 ECDH: derive_bits(priv_bytes, pub_bytes) -> shared_secret or empty on error
    g.set(
        "__braille_crypto_x448_derive_bits",
        Function::new(
            ctx.clone(),
            |priv_bytes: Vec<u8>, pub_bytes: Vec<u8>| -> Vec<u8> {
                let mut priv_arr = [0u8; 56];
                priv_arr.copy_from_slice(&priv_bytes);
                let mut pub_arr = [0u8; 56];
                pub_arr.copy_from_slice(&pub_bytes);

                match x448::x448(priv_arr, pub_arr) {
                    Some(shared) => {
                        if shared.iter().all(|&b| b == 0) {
                            return vec![];
                        }
                        shared.to_vec()
                    }
                    None => vec![],
                }
            },
        )
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

    // Ed25519 get public key from private: (priv_bytes) -> pub_bytes(32)
    g.set(
        "__braille_crypto_ed25519_get_public",
        Function::new(ctx.clone(), |priv_bytes: Vec<u8>| -> Vec<u8> {
            use ed25519_dalek::SigningKey;
            let mut key_arr = [0u8; 32];
            key_arr.copy_from_slice(&priv_bytes);
            let signing_key = SigningKey::from_bytes(&key_arr);
            signing_key.verifying_key().as_bytes().to_vec()
        })
        .unwrap(),
    )
    .unwrap();

    // X25519 get public key from private: (priv_bytes) -> pub_bytes(32)
    g.set(
        "__braille_crypto_x25519_get_public",
        Function::new(ctx.clone(), |priv_bytes: Vec<u8>| -> Vec<u8> {
            use x25519_dalek::{PublicKey, StaticSecret};
            let mut key_arr = [0u8; 32];
            key_arr.copy_from_slice(&priv_bytes);
            let secret = StaticSecret::from(key_arr);
            let public = PublicKey::from(&secret);
            public.as_bytes().to_vec()
        })
        .unwrap(),
    )
    .unwrap();

    // X448 get public key from private: (priv_bytes) -> pub_bytes(56)
    g.set(
        "__braille_crypto_x448_get_public",
        Function::new(ctx.clone(), |priv_bytes: Vec<u8>| -> Vec<u8> {
            let mut priv_arr = [0u8; 56];
            priv_arr.copy_from_slice(&priv_bytes);
            let mut base_point = [0u8; 56];
            base_point[0] = 5;
            let pub_bytes =
                x448::x448(priv_arr, base_point).expect("X448 key derivation failed");
            pub_bytes.to_vec()
        })
        .unwrap(),
    )
    .unwrap();

    // Ed448 get public key from private: (priv_bytes) -> pub_bytes(57)
    g.set(
        "__braille_crypto_ed448_get_public",
        Function::new(ctx.clone(), |priv_bytes: Vec<u8>| -> Vec<u8> {
            let signing_key = ed448_goldilocks::SigningKey::try_from(priv_bytes.as_slice())
                .expect("invalid Ed448 signing key");
            signing_key.verifying_key().to_bytes().to_vec()
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

    // Ed448 key generation: returns [pub_bytes(57), priv_bytes(57)]
    g.set(
        "__braille_crypto_ed448_generate",
        Function::new(ctx.clone(), || -> Vec<Vec<u8>> {
            use ed448_goldilocks::elliptic_curve::Generate;
            let signing_key = ed448_goldilocks::SigningKey::generate();
            let verifying_key = signing_key.verifying_key();
            vec![
                verifying_key.to_bytes().to_vec(),
                signing_key.to_bytes().to_vec(),
            ]
        })
        .unwrap(),
    )
    .unwrap();

    // Ed448 sign: (priv_bytes, data) -> signature(114)
    g.set(
        "__braille_crypto_ed448_sign",
        Function::new(
            ctx.clone(),
            |priv_bytes: Vec<u8>, data: Vec<u8>| -> Vec<u8> {
                let signing_key = ed448_goldilocks::SigningKey::try_from(priv_bytes.as_slice())
                    .expect("invalid Ed448 signing key");
                let sig = signing_key.sign_raw(&data);
                sig.to_bytes().to_vec()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // Ed448 verify: (pub_bytes, signature, data) -> bool
    g.set(
        "__braille_crypto_ed448_verify",
        Function::new(
            ctx.clone(),
            |pub_bytes: Vec<u8>, signature: Vec<u8>, data: Vec<u8>| -> bool {
                let mut pub_arr = [0u8; 57];
                pub_arr.copy_from_slice(&pub_bytes);
                let verifying_key = ed448_goldilocks::VerifyingKey::from_bytes(&pub_arr)
                    .expect("invalid Ed448 verifying key");
                let sig = ed448_goldilocks::Signature::try_from(signature.as_slice())
                    .expect("invalid Ed448 signature");
                verifying_key.verify_raw(&sig, &data).is_ok()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ECDH P-256/P-384/P-521 key generation: (curve) -> [pub_uncompressed, priv_bytes]
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
                "P-521" => {
                    let nz = p521::NonZeroScalar::random(&mut rand_core::OsRng);
                    let public = p521::PublicKey::from_secret_scalar(&nz);
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
                    "P-521" => {
                        use p521::ecdh::diffie_hellman;
                        use p521::elliptic_curve::sec1::FromEncodedPoint;
                        let scalar = p521::NonZeroScalar::from_repr(
                            *p521::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-521 private key");
                        let encoded = p521::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-521 public key encoding");
                        let public = p521::PublicKey::from_encoded_point(&encoded)
                            .expect("invalid P-521 public key point");
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

    // ECDSA sign: (curve, hash, priv_bytes, data) -> signature (r||s fixed-size)
    g.set(
        "__braille_crypto_ecdsa_sign",
        Function::new(
            ctx.clone(),
            |curve: String, hash: String, priv_bytes: Vec<u8>, data: Vec<u8>| -> Vec<u8> {
                use sha1::Sha1;
                use sha2::{Digest, Sha256, Sha384, Sha512};
                // Pre-hash the data with the specified hash
                let digest: Vec<u8> = match hash.as_str() {
                    "SHA-1" => Sha1::digest(&data).to_vec(),
                    "SHA-256" => Sha256::digest(&data).to_vec(),
                    "SHA-384" => Sha384::digest(&data).to_vec(),
                    "SHA-512" => Sha512::digest(&data).to_vec(),
                    other => panic!("NotSupportedError: hash '{other}' not supported"),
                };
                macro_rules! ecdsa_sign {
                    ($mod:ident, $priv:expr, $digest:expr) => {{
                        let sk = $mod::ecdsa::SigningKey::from_bytes(
                            $mod::FieldBytes::from_slice($priv),
                        ).expect(concat!("invalid ", stringify!($mod), " signing key"));
                        let sig: $mod::ecdsa::Signature = sk.sign_prehash($digest)
                            .expect(concat!(stringify!($mod), " sign failed"));
                        sig.to_bytes().to_vec()
                    }};
                }
                match curve.as_str() {
                    "P-256" => ecdsa_sign!(p256, &priv_bytes, &digest),
                    "P-384" => ecdsa_sign!(p384, &priv_bytes, &digest),
                    "P-521" => ecdsa_sign!(p521, &priv_bytes, &digest),
                    other => panic!("NotSupportedError: ECDSA curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // EC PKCS8 import: (curve, der_bytes) -> [priv_scalar, pub_uncompressed] or empty on error
    g.set(
        "__braille_crypto_ec_pkcs8_import",
        Function::new(
            ctx.clone(),
            |curve: String, der_bytes: Vec<u8>| -> Vec<Vec<u8>> {
                match curve.as_str() {
                    "P-256" => {
                        use p256::pkcs8::DecodePrivateKey;
                        match p256::SecretKey::from_pkcs8_der(&der_bytes) {
                            Ok(secret_key) => {
                                let public_key = secret_key.public_key();
                                vec![secret_key.to_bytes().to_vec(), public_key.to_sec1_bytes().to_vec()]
                            }
                            Err(_) => vec![],
                        }
                    }
                    "P-384" => {
                        use p384::pkcs8::DecodePrivateKey;
                        match p384::SecretKey::from_pkcs8_der(&der_bytes) {
                            Ok(secret_key) => {
                                let public_key = secret_key.public_key();
                                vec![secret_key.to_bytes().to_vec(), public_key.to_sec1_bytes().to_vec()]
                            }
                            Err(_) => vec![],
                        }
                    }
                    "P-521" => {
                        use p521::pkcs8::DecodePrivateKey;
                        match p521::SecretKey::from_pkcs8_der(&der_bytes) {
                            Ok(secret_key) => {
                                let public_key = secret_key.public_key();
                                vec![secret_key.to_bytes().to_vec(), public_key.to_sec1_bytes().to_vec()]
                            }
                            Err(_) => vec![],
                        }
                    }
                    _ => vec![],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // EC SPKI import: (der_bytes) -> [curve_name_bytes, pub_uncompressed]
    g.set(
        "__braille_crypto_ec_spki_import",
        Function::new(
            ctx.clone(),
            |der_bytes: Vec<u8>| -> Vec<Vec<u8>> {
                use p256::pkcs8::DecodePublicKey;
                // Try P-256 first
                if let Ok(pk) = p256::PublicKey::from_public_key_der(&der_bytes) {
                    return vec![b"P-256".to_vec(), pk.to_sec1_bytes().to_vec()];
                }
                // Try P-384
                if let Ok(pk) = p384::PublicKey::from_public_key_der(&der_bytes) {
                    return vec![b"P-384".to_vec(), pk.to_sec1_bytes().to_vec()];
                }
                // Try P-521
                if let Ok(pk) = p521::PublicKey::from_public_key_der(&der_bytes) {
                    return vec![b"P-521".to_vec(), pk.to_sec1_bytes().to_vec()];
                }
                vec![] // Return empty on error — JS side rejects with DataError
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
             hash: String,
             pub_bytes: Vec<u8>,
             signature: Vec<u8>,
             data: Vec<u8>|
             -> bool {
                use sha1::Sha1;
                use sha2::{Digest, Sha256, Sha384, Sha512};
                let digest: Vec<u8> = match hash.as_str() {
                    "SHA-1" => Sha1::digest(&data).to_vec(),
                    "SHA-256" => Sha256::digest(&data).to_vec(),
                    "SHA-384" => Sha384::digest(&data).to_vec(),
                    "SHA-512" => Sha512::digest(&data).to_vec(),
                    other => panic!("NotSupportedError: hash '{other}' not supported"),
                };
                macro_rules! ecdsa_verify_prehash {
                    ($mod:ident, $pub_bytes:expr, $signature:expr, $digest:expr) => {{
                        use $mod::ecdsa::{Signature, VerifyingKey};
                        let encoded = $mod::EncodedPoint::from_bytes($pub_bytes)
                            .expect(concat!("invalid ", stringify!($mod), " public key"));
                        let vk = VerifyingKey::from_encoded_point(&encoded)
                            .expect(concat!("invalid ", stringify!($mod), " verifying key"));
                        let sig = match Signature::from_slice($signature) {
                            Ok(s) => s,
                            Err(_) => return false,
                        };
                        vk.verify_prehash($digest, &sig).is_ok()
                    }};
                }
                match curve.as_str() {
                    "P-256" => ecdsa_verify_prehash!(p256, &pub_bytes, &signature, &digest),
                    "P-384" => ecdsa_verify_prehash!(p384, &pub_bytes, &signature, &digest),
                    "P-521" => ecdsa_verify_prehash!(p521, &pub_bytes, &signature, &digest),
                    other => panic!("NotSupportedError: ECDSA curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // EC SPKI export: (curve, pub_uncompressed) -> spki_der
    g.set(
        "__braille_crypto_ec_spki_export",
        Function::new(
            ctx.clone(),
            |curve: String, pub_bytes: Vec<u8>| -> Vec<u8> {
                use p256::pkcs8::EncodePublicKey;
                match curve.as_str() {
                    "P-256" => {
                        use p256::elliptic_curve::sec1::FromEncodedPoint;
                        let ep = p256::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-256 public key");
                        let pk = p256::PublicKey::from_encoded_point(&ep)
                            .expect("invalid P-256 point");
                        pk.to_public_key_der().expect("encode failed").into_vec()
                    }
                    "P-384" => {
                        use p384::elliptic_curve::sec1::FromEncodedPoint;
                        let ep = p384::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-384 public key");
                        let pk = p384::PublicKey::from_encoded_point(&ep)
                            .expect("invalid P-384 point");
                        pk.to_public_key_der().expect("encode failed").into_vec()
                    }
                    "P-521" => {
                        use p521::elliptic_curve::sec1::FromEncodedPoint;
                        let ep = p521::EncodedPoint::from_bytes(&pub_bytes)
                            .expect("invalid P-521 public key");
                        let pk = p521::PublicKey::from_encoded_point(&ep)
                            .expect("invalid P-521 point");
                        pk.to_public_key_der().expect("encode failed").into_vec()
                    }
                    other => panic!("NotSupportedError: SPKI export for curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // EC PKCS8 export: (curve, priv_scalar, pub_uncompressed) -> pkcs8_der
    g.set(
        "__braille_crypto_ec_pkcs8_export",
        Function::new(
            ctx.clone(),
            |curve: String, priv_bytes: Vec<u8>, _pub_bytes: Vec<u8>| -> Vec<u8> {
                use p256::pkcs8::EncodePrivateKey;
                match curve.as_str() {
                    "P-256" => {
                        let sk = p256::SecretKey::from_bytes(
                            p256::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-256 private key");
                        sk.to_pkcs8_der().expect("encode failed").to_bytes().to_vec()
                    }
                    "P-384" => {
                        let sk = p384::SecretKey::from_bytes(
                            p384::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-384 private key");
                        sk.to_pkcs8_der().expect("encode failed").to_bytes().to_vec()
                    }
                    "P-521" => {
                        let sk = p521::SecretKey::from_bytes(
                            p521::FieldBytes::from_slice(&priv_bytes),
                        )
                        .expect("invalid P-521 private key");
                        sk.to_pkcs8_der().expect("encode failed").to_bytes().to_vec()
                    }
                    other => panic!("NotSupportedError: PKCS8 export for curve '{other}' not supported"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // EC point decompress: (curve, compressed_bytes) -> uncompressed_bytes or empty on error
    g.set(
        "__braille_crypto_ec_decompress",
        Function::new(
            ctx.clone(),
            |curve: String, point_bytes: Vec<u8>| -> Vec<u8> {
                macro_rules! try_decompress {
                    ($mod:ident) => {{
                        use $mod::elliptic_curve::sec1::FromEncodedPoint;
                        let ep = match $mod::EncodedPoint::from_bytes(&point_bytes) {
                            Ok(ep) => ep,
                            Err(_) => return vec![],
                        };
                        let opt: Option<$mod::PublicKey> =
                            $mod::PublicKey::from_encoded_point(&ep).into();
                        match opt {
                            Some(pk) => pk.to_sec1_bytes().to_vec(),
                            None => vec![],
                        }
                    }};
                }
                match curve.as_str() {
                    "P-256" => try_decompress!(p256),
                    "P-384" => try_decompress!(p384),
                    "P-521" => try_decompress!(p521),
                    _ => vec![],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // EC JWK export: (curve, pub_uncompressed, priv_scalar_or_empty) -> json_string
    g.set(
        "__braille_crypto_ec_jwk_export",
        Function::new(
            ctx.clone(),
            |curve: String, pub_bytes: Vec<u8>, priv_bytes: Vec<u8>| -> String {
                // pub_bytes is uncompressed SEC1: 04 || x || y
                let coord_len = match curve.as_str() {
                    "P-256" => 32,
                    "P-384" => 48,
                    "P-521" => 66,
                    other => panic!("NotSupportedError: JWK export for curve '{other}' not supported"),
                };
                let crv = curve.as_str();
                // Skip the 04 prefix
                let x = &pub_bytes[1..1 + coord_len];
                let y = &pub_bytes[1 + coord_len..1 + 2 * coord_len];

                let x_b64 = super::utils::b64url_encode(x);
                let y_b64 = super::utils::b64url_encode(y);
                if priv_bytes.is_empty() {
                    format!(r#"{{"kty":"EC","crv":"{}","x":"{}","y":"{}"}}"#, crv, x_b64, y_b64)
                } else {
                    let d_b64 = super::utils::b64url_encode(&priv_bytes);
                    format!(r#"{{"kty":"EC","crv":"{}","x":"{}","y":"{}","d":"{}"}}"#, crv, x_b64, y_b64, d_b64)
                }
            },
        )
        .unwrap(),
    )
    .unwrap();
}
