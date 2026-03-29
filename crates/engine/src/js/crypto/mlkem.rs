use ml_kem::kem::{Decapsulate, Encapsulate, Kem, KeyExport};
use ml_kem::{MlKem1024, MlKem512, MlKem768, Seed};
use rquickjs::{Ctx, Function};

macro_rules! mlkem_ops {
    ($variant:ty, $dk_type:ty, $ek_type:ty) => {
        impl MlKemOps for $variant {
            fn generate() -> Vec<Vec<u8>> {
                let (dk, ek): ($dk_type, $ek_type) = <$variant>::generate_keypair();
                let ek_bytes: Vec<u8> = KeyExport::to_bytes(&ek).to_vec();
                let dk_seed: Seed = dk.to_seed().expect("seed available");
                vec![ek_bytes, dk_seed.to_vec()]
            }

            fn encapsulate(ek_bytes: &[u8]) -> Vec<Vec<u8>> {
                let ek_arr: ml_kem::kem::Key<$ek_type> =
                    ek_bytes.try_into().expect("valid ek length");
                let ek = <$ek_type>::new(&ek_arr).expect("valid encapsulation key");
                let (ct, ss) = ek.encapsulate();
                vec![ct.to_vec(), ss.to_vec()]
            }

            fn decapsulate(dk_seed_bytes: &[u8], ct_bytes: &[u8]) -> Vec<u8> {
                let seed: Seed = dk_seed_bytes.try_into().expect("valid seed length");
                let dk = <$dk_type>::from_seed(seed);
                let ct: ml_kem::kem::Ciphertext<$variant> =
                    ct_bytes.try_into().expect("valid ciphertext length");
                let ss = dk.decapsulate(&ct);
                ss.to_vec()
            }

            fn from_seed(seed_bytes: &[u8]) -> Vec<Vec<u8>> {
                let seed: Seed = seed_bytes.try_into().expect("valid seed length");
                let dk = <$dk_type>::from_seed(seed);
                let ek = dk.encapsulation_key();
                let ek_bytes: Vec<u8> = ek.to_bytes().to_vec();
                let dk_seed = dk.to_seed().expect("seed available");
                vec![ek_bytes, dk_seed.to_vec()]
            }
        }
    };
}

trait MlKemOps {
    fn generate() -> Vec<Vec<u8>>;
    fn encapsulate(ek_bytes: &[u8]) -> Vec<Vec<u8>>;
    fn decapsulate(dk_seed_bytes: &[u8], ct_bytes: &[u8]) -> Vec<u8>;
    fn from_seed(seed_bytes: &[u8]) -> Vec<Vec<u8>>;
}

mlkem_ops!(MlKem512, ml_kem::DecapsulationKey512, ml_kem::EncapsulationKey512);
mlkem_ops!(MlKem768, ml_kem::DecapsulationKey768, ml_kem::EncapsulationKey768);
mlkem_ops!(MlKem1024, ml_kem::DecapsulationKey1024, ml_kem::EncapsulationKey1024);

fn dispatch_generate(variant: &str) -> Vec<Vec<u8>> {
    match variant {
        "ML-KEM-512" => MlKem512::generate(),
        "ML-KEM-768" => MlKem768::generate(),
        "ML-KEM-1024" => MlKem1024::generate(),
        other => panic!("NotSupportedError: ML-KEM variant '{other}' not supported"),
    }
}

fn dispatch_encapsulate(variant: &str, ek_bytes: &[u8]) -> Vec<Vec<u8>> {
    match variant {
        "ML-KEM-512" => MlKem512::encapsulate(ek_bytes),
        "ML-KEM-768" => MlKem768::encapsulate(ek_bytes),
        "ML-KEM-1024" => MlKem1024::encapsulate(ek_bytes),
        other => panic!("NotSupportedError: ML-KEM variant '{other}' not supported"),
    }
}

fn dispatch_decapsulate(variant: &str, dk_seed: &[u8], ct_bytes: &[u8]) -> Vec<u8> {
    match variant {
        "ML-KEM-512" => MlKem512::decapsulate(dk_seed, ct_bytes),
        "ML-KEM-768" => MlKem768::decapsulate(dk_seed, ct_bytes),
        "ML-KEM-1024" => MlKem1024::decapsulate(dk_seed, ct_bytes),
        other => panic!("NotSupportedError: ML-KEM variant '{other}' not supported"),
    }
}

fn dispatch_from_seed(variant: &str, seed_bytes: &[u8]) -> Vec<Vec<u8>> {
    match variant {
        "ML-KEM-512" => MlKem512::from_seed(seed_bytes),
        "ML-KEM-768" => MlKem768::from_seed(seed_bytes),
        "ML-KEM-1024" => MlKem1024::from_seed(seed_bytes),
        other => panic!("NotSupportedError: ML-KEM variant '{other}' not supported"),
    }
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_mlkem_generate",
        Function::new(ctx.clone(), |variant: String| -> Vec<Vec<u8>> {
            dispatch_generate(&variant)
        })
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mlkem_encapsulate",
        Function::new(
            ctx.clone(),
            |variant: String, ek_bytes: Vec<u8>| -> Vec<Vec<u8>> {
                dispatch_encapsulate(&variant, &ek_bytes)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mlkem_decapsulate",
        Function::new(
            ctx.clone(),
            |variant: String, dk_seed: Vec<u8>, ct_bytes: Vec<u8>| -> Vec<u8> {
                dispatch_decapsulate(&variant, &dk_seed, &ct_bytes)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_mlkem_from_seed",
        Function::new(
            ctx.clone(),
            |variant: String, seed_bytes: Vec<u8>| -> Vec<Vec<u8>> {
                dispatch_from_seed(&variant, &seed_bytes)
            },
        )
        .unwrap(),
    )
    .unwrap();
}
