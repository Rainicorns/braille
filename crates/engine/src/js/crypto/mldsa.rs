use ml_dsa::{
    signature::{Signer, Verifier},
    KeyGen, MlDsa44, MlDsa65, MlDsa87, Signature, SigningKey, VerifyingKey,
};
use rquickjs::{Ctx, Function};

macro_rules! mldsa_ops {
    ($variant:ty) => {
        impl MlDsaOps for $variant {
            fn sign(seed: &[u8], data: &[u8]) -> Vec<u8> {
                let seed_arr: ml_dsa::Seed =
                    seed.try_into().expect("invalid ML-DSA seed length");
                let kp = <$variant>::from_seed(&seed_arr);
                let sig = kp.sign(data);
                sig.encode().to_vec()
            }

            fn verify(vk_bytes: &[u8], signature: &[u8], data: &[u8]) -> bool {
                let vk_enc = match ml_dsa::EncodedVerifyingKey::<$variant>::try_from(vk_bytes) {
                    Ok(enc) => enc,
                    Err(_) => return false,
                };
                let vk = VerifyingKey::<$variant>::decode(&vk_enc);
                let sig = match Signature::<$variant>::try_from(signature) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                vk.verify(data, &sig).is_ok()
            }

            fn pkcs8_import(der: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
                use ml_dsa::pkcs8::DecodePrivateKey;
                let sk = SigningKey::<$variant>::from_pkcs8_der(der).ok()?;
                let seed = sk.to_seed().to_vec();
                let vk = ml_dsa::signature::Keypair::verifying_key(&sk);
                let vk_bytes = vk.encode().to_vec();
                Some((seed, vk_bytes))
            }

            fn spki_import(der: &[u8]) -> Option<Vec<u8>> {
                use ml_dsa::pkcs8::DecodePublicKey;
                let vk = VerifyingKey::<$variant>::from_public_key_der(der).ok()?;
                Some(vk.encode().to_vec())
            }

            fn vk_from_seed(seed: &[u8]) -> Option<Vec<u8>> {
                let seed_arr: ml_dsa::Seed = seed.try_into().ok()?;
                let kp = <$variant as KeyGen>::from_seed(&seed_arr);
                let vk = ml_dsa::signature::Keypair::verifying_key(&kp);
                Some(vk.encode().to_vec())
            }
        }
    };
}

trait MlDsaOps {
    fn sign(seed: &[u8], data: &[u8]) -> Vec<u8>;
    fn verify(vk_bytes: &[u8], signature: &[u8], data: &[u8]) -> bool;
    fn pkcs8_import(der: &[u8]) -> Option<(Vec<u8>, Vec<u8>)>;
    fn spki_import(der: &[u8]) -> Option<Vec<u8>>;
    fn vk_from_seed(seed: &[u8]) -> Option<Vec<u8>>;
}

mldsa_ops!(MlDsa44);
mldsa_ops!(MlDsa65);
mldsa_ops!(MlDsa87);

fn dispatch_sign(algo: &str, seed: &[u8], data: &[u8]) -> Vec<u8> {
    match algo {
        "ML-DSA-44" => MlDsa44::sign(seed, data),
        "ML-DSA-65" => MlDsa65::sign(seed, data),
        "ML-DSA-87" => MlDsa87::sign(seed, data),
        other => panic!("NotSupportedError: ML-DSA variant '{other}' not supported"),
    }
}

fn dispatch_verify(algo: &str, vk_bytes: &[u8], signature: &[u8], data: &[u8]) -> bool {
    match algo {
        "ML-DSA-44" => MlDsa44::verify(vk_bytes, signature, data),
        "ML-DSA-65" => MlDsa65::verify(vk_bytes, signature, data),
        "ML-DSA-87" => MlDsa87::verify(vk_bytes, signature, data),
        other => panic!("NotSupportedError: ML-DSA variant '{other}' not supported"),
    }
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_mldsa_sign",
        Function::new(
            ctx.clone(),
            |algo: String, seed: Vec<u8>, data: Vec<u8>| -> Vec<u8> {
                dispatch_sign(&algo, &seed, &data)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mldsa_verify",
        Function::new(
            ctx.clone(),
            |algo: String, vk_bytes: Vec<u8>, signature: Vec<u8>, data: Vec<u8>| -> bool {
                dispatch_verify(&algo, &vk_bytes, &signature, &data)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mldsa_pkcs8_import",
        Function::new(
            ctx.clone(),
            |algo: String, der: Vec<u8>| -> Vec<Vec<u8>> {
                let result = match algo.as_str() {
                    "ML-DSA-44" => MlDsa44::pkcs8_import(&der),
                    "ML-DSA-65" => MlDsa65::pkcs8_import(&der),
                    "ML-DSA-87" => MlDsa87::pkcs8_import(&der),
                    _ => None,
                };
                match result {
                    Some((seed, vk)) => vec![seed, vk],
                    None => vec![],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mldsa_from_seed",
        Function::new(
            ctx.clone(),
            |algo: String, seed: Vec<u8>| -> Vec<u8> {
                let result = match algo.as_str() {
                    "ML-DSA-44" => MlDsa44::vk_from_seed(&seed),
                    "ML-DSA-65" => MlDsa65::vk_from_seed(&seed),
                    "ML-DSA-87" => MlDsa87::vk_from_seed(&seed),
                    _ => None,
                };
                result.unwrap_or_default()
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mldsa_spki_import",
        Function::new(
            ctx.clone(),
            |algo: String, der: Vec<u8>| -> Vec<u8> {
                let result = match algo.as_str() {
                    "ML-DSA-44" => MlDsa44::spki_import(&der),
                    "ML-DSA-65" => MlDsa65::spki_import(&der),
                    "ML-DSA-87" => MlDsa87::spki_import(&der),
                    _ => None,
                };
                result.unwrap_or_default()
            },
        )
        .unwrap(),
    )
    .unwrap();
}
