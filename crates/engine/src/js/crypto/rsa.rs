use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // RSA SPKI import: parse DER, return [der_bytes, modulus_bits_bytes, pub_exp_bytes]
    g.set(
        "__braille_crypto_rsa_spki_import",
        Function::new(ctx.clone(), |der: Vec<u8>| -> Vec<Vec<u8>> {
            use rsa::pkcs8::DecodePublicKey;
            use rsa::traits::PublicKeyParts;
            let pk = rsa::RsaPublicKey::from_public_key_der(&der)
                .expect("DataError: invalid RSA SPKI DER");
            let mod_bits = (pk.n().bits() as u32).to_be_bytes().to_vec();
            let pub_exp = pk.e().to_bytes_be();
            vec![der, mod_bits, pub_exp]
        })
        .unwrap(),
    )
    .unwrap();

    // RSA PKCS8 import: parse DER, return [priv_der, pub_spki_der, modulus_bits_bytes, pub_exp_bytes]
    g.set(
        "__braille_crypto_rsa_pkcs8_import",
        Function::new(ctx.clone(), |der: Vec<u8>| -> Vec<Vec<u8>> {
            use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
            use rsa::traits::PublicKeyParts;
            let private_key = rsa::RsaPrivateKey::from_pkcs8_der(&der)
                .expect("DataError: invalid RSA PKCS8 DER");
            let public_key = private_key.to_public_key();
            let pub_der = public_key
                .to_public_key_der()
                .expect("failed to encode RSA public key")
                .into_vec();
            let mod_bits = (public_key.n().bits() as u32).to_be_bytes().to_vec();
            let pub_exp = public_key.e().to_bytes_be();
            vec![der, pub_der, mod_bits, pub_exp]
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

                fn b64url(data: &[u8]) -> String {
                    use std::fmt::Write;
                    const TABLE: &[u8] =
                        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
                    let mut out = String::new();
                    let mut i = 0;
                    while i < data.len() {
                        let b0 = data[i] as u32;
                        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
                        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
                        let triple = (b0 << 16) | (b1 << 8) | b2;
                        let _ = write!(out, "{}", TABLE[(triple >> 18 & 0x3F) as usize] as char);
                        let _ = write!(out, "{}", TABLE[(triple >> 12 & 0x3F) as usize] as char);
                        if i + 1 < data.len() {
                            let _ =
                                write!(out, "{}", TABLE[(triple >> 6 & 0x3F) as usize] as char);
                        }
                        if i + 2 < data.len() {
                            let _ = write!(out, "{}", TABLE[(triple & 0x3F) as usize] as char);
                        }
                        i += 3;
                    }
                    out
                }

                let pk = rsa::RsaPublicKey::from_public_key_der(&pub_der)
                    .expect("invalid RSA public key");
                let n = b64url(&pk.n().to_bytes_be());
                let e = b64url(&pk.e().to_bytes_be());

                if priv_der.is_empty() {
                    format!(r#"{{"kty":"RSA","n":"{}","e":"{}"}}"#, n, e)
                } else {
                    use rsa::pkcs8::DecodePrivateKey;
                    use rsa::traits::PrivateKeyParts;
                    let sk = rsa::RsaPrivateKey::from_pkcs8_der(&priv_der)
                        .expect("invalid RSA private key");
                    let d = b64url(&sk.d().to_bytes_be());
                    let primes = sk.primes();
                    let p = b64url(&primes[0].to_bytes_be());
                    let q = b64url(&primes[1].to_bytes_be());
                    let dp = sk
                        .dp()
                        .map(|v| b64url(&v.to_bytes_be()))
                        .unwrap_or_default();
                    let dq = sk
                        .dq()
                        .map(|v| b64url(&v.to_bytes_be()))
                        .unwrap_or_default();
                    let qi = sk
                        .qinv()
                        .map(|v| {
                            let (_, bytes) = v.to_bytes_be();
                            b64url(&bytes)
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
}
