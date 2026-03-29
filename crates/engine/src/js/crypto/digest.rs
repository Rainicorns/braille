use rquickjs::{Ctx, Function};
use sha2::{Digest, Sha256};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_digest",
        Function::new(ctx.clone(), |algo: String, data: Vec<u8>| -> Vec<u8> {
            match algo.as_str() {
                "SHA-256" => {
                    let mut hasher = Sha256::new();
                    hasher.update(&data);
                    hasher.finalize().to_vec()
                }
                "SHA-1" => ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &data)
                    .as_ref()
                    .to_vec(),
                "SHA-384" => ring::digest::digest(&ring::digest::SHA384, &data)
                    .as_ref()
                    .to_vec(),
                "SHA-512" => ring::digest::digest(&ring::digest::SHA512, &data)
                    .as_ref()
                    .to_vec(),
                other => panic!("NotSupportedError: digest algorithm '{other}' not supported"),
            }
        })
        .unwrap(),
    )
    .unwrap();
}
