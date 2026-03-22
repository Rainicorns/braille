//! Real WebCrypto implementation backed by `ring`.
//!
//! Exposes native Rust functions to QuickJS that perform actual cryptographic
//! operations. The JS layer in globals.rs wraps these into the standard
//! `crypto.subtle` API surface (returning Promises).

use rquickjs::{Ctx, Function};

/// Register all native crypto helper functions on the global object.
pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // __braille_crypto_get_random_bytes(len) -> Vec<u8>
    g.set(
        "__braille_crypto_get_random_bytes",
        Function::new(ctx.clone(), |len: u32| -> Vec<u8> {
            let mut buf = vec![0u8; len as usize];
            getrandom::getrandom(&mut buf).expect("getrandom failed");
            buf
        })
        .unwrap(),
    )
    .unwrap();

    // __braille_crypto_digest(algo, data) -> Vec<u8>
    g.set(
        "__braille_crypto_digest",
        Function::new(ctx.clone(), |algo: String, data: Vec<u8>| -> Vec<u8> {
            let algorithm = match algo.as_str() {
                "SHA-1" => &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
                "SHA-256" => &ring::digest::SHA256,
                "SHA-384" => &ring::digest::SHA384,
                "SHA-512" => &ring::digest::SHA512,
                other => panic!("NotSupportedError: digest algorithm '{other}' not supported"),
            };
            ring::digest::digest(algorithm, &data).as_ref().to_vec()
        })
        .unwrap(),
    )
    .unwrap();

    // __braille_crypto_aes_gcm_encrypt(key_bytes, iv_bytes, plaintext, aad) -> Vec<u8>
    g.set(
        "__braille_crypto_aes_gcm_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, plaintext: Vec<u8>, aad: Vec<u8>| -> Vec<u8> {
                use ring::aead;
                let algo = match key.len() {
                    16 => &aead::AES_128_GCM,
                    32 => &aead::AES_256_GCM,
                    _ => panic!("OperationError: AES key must be 128 or 256 bits"),
                };
                let unbound_key = aead::UnboundKey::new(algo, &key).expect("bad AES key");
                let nonce = aead::Nonce::try_assume_unique_for_key(&iv)
                    .expect("OperationError: IV must be 12 bytes");
                let sealing_key = aead::LessSafeKey::new(unbound_key);
                let mut in_out = plaintext;
                sealing_key
                    .seal_in_place_append_tag(nonce, aead::Aad::from(&aad), &mut in_out)
                    .expect("AES-GCM seal failed");
                in_out
            },
        )
        .unwrap(),
    )
    .unwrap();

    // __braille_crypto_aes_gcm_decrypt(key_bytes, iv_bytes, ciphertext_with_tag, aad) -> Vec<u8>
    g.set(
        "__braille_crypto_aes_gcm_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, ciphertext: Vec<u8>, aad: Vec<u8>| -> Vec<u8> {
                use ring::aead;
                let algo = match key.len() {
                    16 => &aead::AES_128_GCM,
                    32 => &aead::AES_256_GCM,
                    _ => panic!("OperationError: AES key must be 128 or 256 bits"),
                };
                let unbound_key = aead::UnboundKey::new(algo, &key).expect("bad AES key");
                let nonce = aead::Nonce::try_assume_unique_for_key(&iv)
                    .expect("OperationError: IV must be 12 bytes");
                let opening_key = aead::LessSafeKey::new(unbound_key);
                let mut in_out = ciphertext;
                let plaintext = opening_key
                    .open_in_place(nonce, aead::Aad::from(&aad), &mut in_out)
                    .expect("OperationError: AES-GCM decryption failed");
                plaintext.to_vec()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // __braille_crypto_hmac_sign(algo, key_bytes, data) -> Vec<u8>
    g.set(
        "__braille_crypto_hmac_sign",
        Function::new(
            ctx.clone(),
            |algo: String, key: Vec<u8>, data: Vec<u8>| -> Vec<u8> {
                let algorithm = match algo.as_str() {
                    "SHA-1" => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
                    "SHA-256" => ring::hmac::HMAC_SHA256,
                    "SHA-384" => ring::hmac::HMAC_SHA384,
                    "SHA-512" => ring::hmac::HMAC_SHA512,
                    other => panic!("NotSupportedError: HMAC hash '{other}' not supported"),
                };
                let signing_key = ring::hmac::Key::new(algorithm, &key);
                ring::hmac::sign(&signing_key, &data).as_ref().to_vec()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // __braille_crypto_hmac_verify(algo, key_bytes, signature, data) -> bool
    g.set(
        "__braille_crypto_hmac_verify",
        Function::new(
            ctx.clone(),
            |algo: String, key: Vec<u8>, signature: Vec<u8>, data: Vec<u8>| -> bool {
                let algorithm = match algo.as_str() {
                    "SHA-1" => ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
                    "SHA-256" => ring::hmac::HMAC_SHA256,
                    "SHA-384" => ring::hmac::HMAC_SHA384,
                    "SHA-512" => ring::hmac::HMAC_SHA512,
                    other => panic!("NotSupportedError: HMAC hash '{other}' not supported"),
                };
                let verification_key = ring::hmac::Key::new(algorithm, &key);
                ring::hmac::verify(&verification_key, &data, &signature).is_ok()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // __braille_crypto_pbkdf2(algo, password, salt, iterations, key_len) -> Vec<u8>
    g.set(
        "__braille_crypto_pbkdf2",
        Function::new(
            ctx.clone(),
            |algo: String,
             password: Vec<u8>,
             salt: Vec<u8>,
             iterations: u32,
             key_len: u32|
             -> Vec<u8> {
                let algorithm = match algo.as_str() {
                    "SHA-1" => ring::pbkdf2::PBKDF2_HMAC_SHA1,
                    "SHA-256" => ring::pbkdf2::PBKDF2_HMAC_SHA256,
                    "SHA-384" => ring::pbkdf2::PBKDF2_HMAC_SHA384,
                    "SHA-512" => ring::pbkdf2::PBKDF2_HMAC_SHA512,
                    other => panic!("NotSupportedError: PBKDF2 hash '{other}' not supported"),
                };
                let mut out = vec![0u8; key_len as usize];
                ring::pbkdf2::derive(
                    algorithm,
                    std::num::NonZeroU32::new(iterations).expect("iterations must be > 0"),
                    &salt,
                    &password,
                    &mut out,
                );
                out
            },
        )
        .unwrap(),
    )
    .unwrap();
}
