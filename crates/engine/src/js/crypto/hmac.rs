use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

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
}
