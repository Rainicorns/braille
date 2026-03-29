use rquickjs::{Ctx, Function};
use sha3::digest::{ExtendableOutput, FixedOutput, HashMarker, Update, XofReader};

fn hash_fixed<H: Default + Update + FixedOutput + HashMarker>(data: &[u8]) -> Vec<u8> {
    let mut hasher = H::default();
    Update::update(&mut hasher, data);
    hasher.finalize_fixed().to_vec()
}

fn hash_xof<H: Default + Update + ExtendableOutput>(data: &[u8], output_bytes: usize) -> Vec<u8> {
    if output_bytes == 0 {
        return vec![];
    }
    let mut hasher = H::default();
    Update::update(&mut hasher, data);
    let mut reader = hasher.finalize_xof();
    let mut result = vec![0u8; output_bytes];
    reader.read(&mut result);
    result
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_digest",
        Function::new(
            ctx.clone(),
            |algo: String, data: Vec<u8>, output_len: u32| -> Vec<u8> {
                match algo.as_str() {
                    "SHA-256" => hash_fixed::<sha2::Sha256>(&data),
                    "SHA-1" => {
                        ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &data)
                            .as_ref()
                            .to_vec()
                    }
                    "SHA-384" => ring::digest::digest(&ring::digest::SHA384, &data)
                        .as_ref()
                        .to_vec(),
                    "SHA-512" => ring::digest::digest(&ring::digest::SHA512, &data)
                        .as_ref()
                        .to_vec(),
                    "SHA3-256" => hash_fixed::<sha3::Sha3_256>(&data),
                    "SHA3-384" => hash_fixed::<sha3::Sha3_384>(&data),
                    "SHA3-512" => hash_fixed::<sha3::Sha3_512>(&data),
                    "CSHAKE128" => hash_xof::<sha3::Shake128>(&data, (output_len / 8) as usize),
                    "CSHAKE256" => hash_xof::<sha3::Shake256>(&data, (output_len / 8) as usize),
                    other => {
                        panic!("NotSupportedError: digest algorithm '{other}' not supported")
                    }
                }
            },
        )
        .unwrap(),
    )
    .unwrap();
}
