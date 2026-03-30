use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // RSA SPKI import: parse DER, return [der_bytes, modulus_bits_bytes, pub_exp_bytes] or empty on error
    g.set(
        "__braille_crypto_rsa_spki_import",
        Function::new(ctx.clone(), |der: Vec<u8>| -> Vec<Vec<u8>> {
            use rsa::pkcs8::DecodePublicKey;
            use rsa::traits::PublicKeyParts;
            match rsa::RsaPublicKey::from_public_key_der(&der) {
                Ok(pk) => {
                    let mod_bits = (pk.n().bits() as u32).to_be_bytes().to_vec();
                    let pub_exp = pk.e().to_bytes_be();
                    vec![der, mod_bits, pub_exp]
                }
                Err(_) => vec![],
            }
        })
        .unwrap(),
    )
    .unwrap();

    // RSA PKCS8 import: parse DER, return [priv_der, pub_spki_der, modulus_bits_bytes, pub_exp_bytes] or empty
    g.set(
        "__braille_crypto_rsa_pkcs8_import",
        Function::new(ctx.clone(), |der: Vec<u8>| -> Vec<Vec<u8>> {
            use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
            use rsa::traits::PublicKeyParts;
            match rsa::RsaPrivateKey::from_pkcs8_der(&der) {
                Ok(private_key) => {
                    let public_key = private_key.to_public_key();
                    let pub_der = public_key
                        .to_public_key_der()
                        .expect("failed to encode RSA public key")
                        .into_vec();
                    let mod_bits = (public_key.n().bits() as u32).to_be_bytes().to_vec();
                    let pub_exp = public_key.e().to_bytes_be();
                    vec![der, pub_der, mod_bits, pub_exp]
                }
                Err(_) => vec![],
            }
        })
        .unwrap(),
    )
    .unwrap();

    // RSA-OAEP encrypt: (pub_der, hash, label, plaintext) -> [1, ciphertext] or [0]
    g.set(
        "__braille_crypto_rsa_oaep_encrypt",
        Function::new(
            ctx.clone(),
            |pub_der: Vec<u8>,
             hash: String,
             label: Vec<u8>,
             plaintext: Vec<u8>|
             -> Vec<Vec<u8>> {
                use rsa::pkcs8::DecodePublicKey;
                use rsa::Oaep;
                let public_key = rsa::RsaPublicKey::from_public_key_der(&pub_der)
                    .expect("invalid RSA public key");
                let mut rng = rand_core::OsRng;
                let lab = if label.is_empty() {
                    None
                } else {
                    Some(String::from_utf8_lossy(&label).to_string())
                };
                let result = match hash.as_str() {
                    "SHA-1" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha1::Sha1, _>(l),
                            None => Oaep::new::<sha1::Sha1>(),
                        };
                        public_key.encrypt(&mut rng, padding, &plaintext)
                    }
                    "SHA-256" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha2::Sha256, _>(l),
                            None => Oaep::new::<sha2::Sha256>(),
                        };
                        public_key.encrypt(&mut rng, padding, &plaintext)
                    }
                    "SHA-384" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha2::Sha384, _>(l),
                            None => Oaep::new::<sha2::Sha384>(),
                        };
                        public_key.encrypt(&mut rng, padding, &plaintext)
                    }
                    "SHA-512" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha2::Sha512, _>(l),
                            None => Oaep::new::<sha2::Sha512>(),
                        };
                        public_key.encrypt(&mut rng, padding, &plaintext)
                    }
                    other => panic!("NotSupportedError: hash '{other}' not supported for RSA-OAEP"),
                };
                match result {
                    Ok(ct) => vec![vec![1], ct],
                    Err(_) => vec![vec![0]],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSA-OAEP decrypt: (priv_der, hash, label, ciphertext) -> [1, plaintext] or [0]
    g.set(
        "__braille_crypto_rsa_oaep_decrypt",
        Function::new(
            ctx.clone(),
            |priv_der: Vec<u8>,
             hash: String,
             label: Vec<u8>,
             ciphertext: Vec<u8>|
             -> Vec<Vec<u8>> {
                use rsa::pkcs8::DecodePrivateKey;
                use rsa::Oaep;
                let private_key = rsa::RsaPrivateKey::from_pkcs8_der(&priv_der)
                    .expect("invalid RSA private key");
                let lab = if label.is_empty() {
                    None
                } else {
                    Some(String::from_utf8_lossy(&label).to_string())
                };
                let result = match hash.as_str() {
                    "SHA-1" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha1::Sha1, _>(l),
                            None => Oaep::new::<sha1::Sha1>(),
                        };
                        private_key.decrypt(padding, &ciphertext)
                    }
                    "SHA-256" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha2::Sha256, _>(l),
                            None => Oaep::new::<sha2::Sha256>(),
                        };
                        private_key.decrypt(padding, &ciphertext)
                    }
                    "SHA-384" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha2::Sha384, _>(l),
                            None => Oaep::new::<sha2::Sha384>(),
                        };
                        private_key.decrypt(padding, &ciphertext)
                    }
                    "SHA-512" => {
                        let padding = match lab {
                            Some(l) => Oaep::new_with_label::<sha2::Sha512, _>(l),
                            None => Oaep::new::<sha2::Sha512>(),
                        };
                        private_key.decrypt(padding, &ciphertext)
                    }
                    other => panic!("NotSupportedError: hash '{other}' not supported for RSA-OAEP"),
                };
                match result {
                    Ok(plaintext) => vec![vec![1], plaintext],
                    Err(_) => vec![vec![0]],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSA JWK export: (pub_der, priv_der_or_empty) -> json_string
    g.set(
        "__braille_crypto_rsa_jwk_export",
        Function::new(
            ctx.clone(),
            |pub_der: Vec<u8>, priv_der: Vec<u8>| -> String {
                use rsa::pkcs8::DecodePublicKey;
                use rsa::traits::PublicKeyParts;

                let pk = rsa::RsaPublicKey::from_public_key_der(&pub_der)
                    .expect("invalid RSA public key");
                let n = super::utils::b64url_encode(&pk.n().to_bytes_be());
                let e = super::utils::b64url_encode(&pk.e().to_bytes_be());

                if priv_der.is_empty() {
                    format!(r#"{{"kty":"RSA","n":"{}","e":"{}"}}"#, n, e)
                } else {
                    use rsa::pkcs8::DecodePrivateKey;
                    use rsa::traits::PrivateKeyParts;
                    let sk = rsa::RsaPrivateKey::from_pkcs8_der(&priv_der)
                        .expect("invalid RSA private key");
                    let d = super::utils::b64url_encode(&sk.d().to_bytes_be());
                    let primes = sk.primes();
                    let p = super::utils::b64url_encode(&primes[0].to_bytes_be());
                    let q = super::utils::b64url_encode(&primes[1].to_bytes_be());
                    let dp = sk
                        .dp()
                        .map(|v| super::utils::b64url_encode(&v.to_bytes_be()))
                        .unwrap_or_default();
                    let dq = sk
                        .dq()
                        .map(|v| super::utils::b64url_encode(&v.to_bytes_be()))
                        .unwrap_or_default();
                    let qi = sk
                        .qinv()
                        .map(|v| {
                            let (_, bytes) = v.to_bytes_be();
                            super::utils::b64url_encode(&bytes)
                        })
                        .unwrap_or_default();
                    format!(
                        r#"{{"kty":"RSA","n":"{}","e":"{}","d":"{}","p":"{}","q":"{}","dp":"{}","dq":"{}","qi":"{}"}}"#,
                        n, e, d, p, q, dp, dq, qi
                    )
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSA key generation: (modulus_bits, pub_exp_bytes) -> [pub_spki_der, priv_pkcs8_der]
    g.set(
        "__braille_crypto_rsa_generate",
        Function::new(
            ctx.clone(),
            |modulus_bits: u32, pub_exp_bytes: Vec<u8>| -> Vec<Vec<u8>> {
                use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
                use rsa::BigUint;
                let exp = BigUint::from_bytes_be(&pub_exp_bytes);
                let mut rng = rand_core::OsRng;
                let private_key =
                    rsa::RsaPrivateKey::new_with_exp(&mut rng, modulus_bits as usize, &exp)
                        .expect("OperationError: RSA key generation failed");
                let public_key = private_key.to_public_key();
                let pub_der = public_key
                    .to_public_key_der()
                    .expect("failed to encode public key")
                    .into_vec();
                let priv_der = private_key
                    .to_pkcs8_der()
                    .expect("failed to encode private key")
                    .to_bytes()
                    .to_vec();
                vec![pub_der, priv_der]
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSA JWK import (private): (json_string) -> [priv_der, pub_der, mod_bits, pub_exp]
    g.set(
        "__braille_crypto_rsa_jwk_import",
        Function::new(
            ctx.clone(),
            |json: String| -> Vec<Vec<u8>> {
                use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
                use rsa::traits::PublicKeyParts;
                use rsa::BigUint;

                let v: serde_json::Value =
                    serde_json::from_str(&json).expect("invalid JSON");
                let n = v["n"].as_str().unwrap_or("");
                let e = v["e"].as_str().unwrap_or("");
                let d = v["d"].as_str().unwrap_or("");
                let p = v["p"].as_str().unwrap_or("");
                let q = v["q"].as_str().unwrap_or("");

                let n_int = BigUint::from_bytes_be(&b64url_decode(n));
                let e_int = BigUint::from_bytes_be(&b64url_decode(e));
                let d_int = BigUint::from_bytes_be(&b64url_decode(d));

                let primes: Vec<BigUint> = if !p.is_empty() && !q.is_empty() {
                    vec![
                        BigUint::from_bytes_be(&b64url_decode(p)),
                        BigUint::from_bytes_be(&b64url_decode(q)),
                    ]
                } else {
                    vec![]
                };

                let private_key =
                    rsa::RsaPrivateKey::from_components(n_int, e_int, d_int, primes)
                        .expect("DataError: invalid RSA JWK components");
                let public_key = private_key.to_public_key();
                let pub_der = public_key
                    .to_public_key_der()
                    .expect("failed to encode public key")
                    .into_vec();
                let priv_der = private_key
                    .to_pkcs8_der()
                    .expect("failed to encode private key")
                    .to_bytes()
                    .to_vec();
                let mod_bits = (public_key.n().bits() as u32).to_be_bytes().to_vec();
                let pub_exp = public_key.e().to_bytes_be();
                vec![priv_der, pub_der, mod_bits, pub_exp]
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSA JWK public import: (n, e) -> [pub_der, mod_bits, pub_exp]
    g.set(
        "__braille_crypto_rsa_jwk_pub_import",
        Function::new(ctx.clone(), |n: String, e: String| -> Vec<Vec<u8>> {
            use rsa::pkcs8::EncodePublicKey;
            use rsa::traits::PublicKeyParts;
            use rsa::BigUint;

            let n_bytes = b64url_decode(&n);
            let e_bytes = b64url_decode(&e);
            let n_int = BigUint::from_bytes_be(&n_bytes);
            let e_int = BigUint::from_bytes_be(&e_bytes);

            let public_key =
                rsa::RsaPublicKey::new(n_int, e_int).expect("DataError: invalid RSA JWK");
            let pub_der = public_key
                .to_public_key_der()
                .expect("failed to encode public key")
                .into_vec();
            let mod_bits = (public_key.n().bits() as u32).to_be_bytes().to_vec();
            let pub_exp = public_key.e().to_bytes_be();
            vec![pub_der, mod_bits, pub_exp]
        })
        .unwrap(),
    )
    .unwrap();

    // Macro to dispatch across hash algorithms — expands to match arms with the hash as a type param
    macro_rules! match_hash {
        ($hash:expr, $err_ctx:expr, |$T:ident| $body:expr) => {
            match $hash {
                "SHA-1" => { type $T = sha1::Sha1; $body }
                "SHA-256" => { type $T = sha2::Sha256; $body }
                "SHA-384" => { type $T = sha2::Sha384; $body }
                "SHA-512" => { type $T = sha2::Sha512; $body }
                other => panic!("NotSupportedError: hash '{other}' not supported for {}", $err_ctx),
            }
        };
    }

    // RSA-PSS sign: (priv_der, hash, salt_length, data) -> signature
    g.set(
        "__braille_crypto_rsa_pss_sign",
        Function::new(
            ctx.clone(),
            |priv_der: Vec<u8>, hash: String, salt_len: u32, data: Vec<u8>| -> Vec<u8> {
                use rsa::pkcs8::DecodePrivateKey;
                use rsa::signature::{RandomizedSigner, SignatureEncoding};
                let private_key = rsa::RsaPrivateKey::from_pkcs8_der(&priv_der)
                    .expect("invalid RSA private key");
                let mut rng = rand_core::OsRng;
                match_hash!(hash.as_str(), "RSA-PSS", |H| {
                    let sk = rsa::pss::BlindedSigningKey::<H>::new_with_salt_len(private_key, salt_len as usize);
                    sk.sign_with_rng(&mut rng, &data).to_vec()
                })
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSA-PSS verify: (pub_der, hash, salt_length, signature, data) -> bool
    g.set(
        "__braille_crypto_rsa_pss_verify",
        Function::new(
            ctx.clone(),
            |pub_der: Vec<u8>, hash: String, salt_len: u32, signature: Vec<u8>, data: Vec<u8>| -> bool {
                use rsa::pkcs8::DecodePublicKey;
                use rsa::signature::Verifier;
                let public_key = rsa::RsaPublicKey::from_public_key_der(&pub_der)
                    .expect("invalid RSA public key");
                let sig = rsa::pss::Signature::try_from(signature.as_slice()).expect("invalid signature");
                match_hash!(hash.as_str(), "RSA-PSS", |H| {
                    rsa::pss::VerifyingKey::<H>::new_with_salt_len(public_key, salt_len as usize).verify(&data, &sig).is_ok()
                })
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSASSA-PKCS1-v1_5 sign: (priv_der, hash, data) -> signature
    g.set(
        "__braille_crypto_rsa_pkcs1_sign",
        Function::new(
            ctx.clone(),
            |priv_der: Vec<u8>, hash: String, data: Vec<u8>| -> Vec<u8> {
                use rsa::pkcs8::DecodePrivateKey;
                use rsa::signature::{Signer, SignatureEncoding};
                let private_key = rsa::RsaPrivateKey::from_pkcs8_der(&priv_der)
                    .expect("invalid RSA private key");
                match_hash!(hash.as_str(), "RSASSA-PKCS1-v1_5", |H| {
                    rsa::pkcs1v15::SigningKey::<H>::new(private_key).sign(&data).to_vec()
                })
            },
        )
        .unwrap(),
    )
    .unwrap();

    // RSASSA-PKCS1-v1_5 verify: (pub_der, hash, signature, data) -> bool
    g.set(
        "__braille_crypto_rsa_pkcs1_verify",
        Function::new(
            ctx.clone(),
            |pub_der: Vec<u8>, hash: String, signature: Vec<u8>, data: Vec<u8>| -> bool {
                use rsa::pkcs8::DecodePublicKey;
                use rsa::signature::Verifier;
                let public_key = rsa::RsaPublicKey::from_public_key_der(&pub_der)
                    .expect("invalid RSA public key");
                let sig = rsa::pkcs1v15::Signature::try_from(signature.as_slice()).expect("invalid signature");
                match_hash!(hash.as_str(), "RSASSA-PKCS1-v1_5", |H| {
                    rsa::pkcs1v15::VerifyingKey::<H>::new(public_key).verify(&data, &sig).is_ok()
                })
            },
        )
        .unwrap(),
    )
    .unwrap();
}

fn b64url_decode(input: &str) -> Vec<u8> {
    super::utils::b64url_decode_url(input)
}
