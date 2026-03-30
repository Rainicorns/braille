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

    // ML-KEM PKCS8 import: parse DER, extract 64-byte seed
    g.set(
        "__braille_crypto_mlkem_pkcs8_import",
        Function::new(
            ctx.clone(),
            |variant: String, der_bytes: Vec<u8>| -> Vec<u8> {
                // ML-KEM PKCS8 structure:
                // SEQUENCE {
                //   INTEGER 0
                //   SEQUENCE { OID (ml-kem variant) }
                //   OCTET STRING { OCTET STRING { 64-byte seed } }
                // }
                // The seed is wrapped in two OCTET STRINGs at the end.
                // Find the inner OCTET STRING containing the 64-byte seed.
                let oid_bytes = mlkem_oid(&variant);
                // Verify the OID is present
                let oid_pos = find_bytes(&der_bytes, &oid_bytes);
                if oid_pos.is_none() {
                    panic!("DataError: ML-KEM PKCS8 OID mismatch for {variant}");
                }
                // ML-KEM PKCS8 format:
                // 30 54 02 01 00 30 0b 06 09 <oid> 04 42 80 40 <64-byte seed>
                // The outer OCTET STRING (04 42) contains a context-specific [0] (80 40) wrapping the seed.
                // Look for 80 40 pattern (context tag 0, length 64)
                let seed_marker = find_last_bytes(&der_bytes, &[0x80, 0x40]);
                if let Some(pos) = seed_marker {
                    if pos + 2 + 64 <= der_bytes.len() {
                        return der_bytes[pos + 2..pos + 2 + 64].to_vec();
                    }
                }
                // Also try standard OCTET STRING wrapping (04 40)
                let seed_marker2 = find_last_bytes(&der_bytes, &[0x04, 0x40]);
                if let Some(pos) = seed_marker2 {
                    if pos + 2 + 64 <= der_bytes.len() {
                        return der_bytes[pos + 2..pos + 2 + 64].to_vec();
                    }
                }
                panic!("DataError: could not extract seed from ML-KEM PKCS8");
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ML-KEM SPKI import: parse DER, extract public key bytes
    g.set(
        "__braille_crypto_mlkem_spki_import",
        Function::new(
            ctx.clone(),
            |variant: String, der_bytes: Vec<u8>| -> Vec<u8> {
                let oid_bytes = mlkem_oid(&variant);
                let oid_pos = find_bytes(&der_bytes, &oid_bytes);
                if oid_pos.is_none() {
                    panic!("DataError: ML-KEM SPKI OID mismatch for {variant}");
                }
                let ek_len = mlkem_ek_len(&variant);
                // SPKI: SEQUENCE { SEQUENCE { OID }, BIT STRING { 0x00, ek_bytes } }
                // The public key is at the end of the DER
                if der_bytes.len() < ek_len {
                    panic!("DataError: ML-KEM SPKI too short");
                }
                // BIT STRING starts with 0x03, then length, then 0x00, then the key bytes
                // The key bytes are the last ek_len bytes
                der_bytes[der_bytes.len() - ek_len..].to_vec()
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ML-KEM PKCS8 export: wrap seed in proper DER PKCS8 structure
    g.set(
        "__braille_crypto_mlkem_pkcs8_export",
        Function::new(
            ctx.clone(),
            |variant: String, seed_bytes: Vec<u8>| -> Vec<u8> {
                let oid = mlkem_oid(&variant);
                // PKCS8: SEQUENCE {
                //   INTEGER 0,
                //   SEQUENCE { OID },
                //   OCTET STRING { [0] seed }
                // }
                // Inner uses context-specific tag [0] (0x80) wrapping the seed
                let mut inner = vec![0x80];
                inner.extend_from_slice(&asn1_length(seed_bytes.len()));
                inner.extend_from_slice(&seed_bytes);
                let outer_octet = asn1_octet_string(&inner); // 04 42 80 40 <seed>
                let algo_seq = asn1_sequence(&asn1_oid(&oid)); // 30 xx 06 xx <oid>
                let version = vec![0x02, 0x01, 0x00]; // INTEGER 0
                let mut body = Vec::new();
                body.extend_from_slice(&version);
                body.extend_from_slice(&algo_seq);
                body.extend_from_slice(&outer_octet);
                asn1_sequence(&body)
            },
        )
        .unwrap(),
    )
    .unwrap();

    // ML-KEM SPKI export: wrap public key in proper DER SPKI structure
    g.set(
        "__braille_crypto_mlkem_spki_export",
        Function::new(
            ctx.clone(),
            |variant: String, pub_bytes: Vec<u8>| -> Vec<u8> {
                let oid = mlkem_oid(&variant);
                // SPKI: SEQUENCE {
                //   SEQUENCE { OID },
                //   BIT STRING { 0x00, pub_bytes }
                // }
                let algo_seq = asn1_sequence(&asn1_oid(&oid));
                let bit_string = asn1_bit_string(&pub_bytes);
                let mut body = Vec::new();
                body.extend_from_slice(&algo_seq);
                body.extend_from_slice(&bit_string);
                asn1_sequence(&body)
            },
        )
        .unwrap(),
    )
    .unwrap();
}

fn mlkem_oid(variant: &str) -> Vec<u8> {
    // 2.16.840.1.101.3.4.4.{1,2,3}
    // Encoded: 60 86 48 01 65 03 04 04 {01,02,03}
    match variant {
        "ML-KEM-512" => vec![0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x04, 0x01],
        "ML-KEM-768" => vec![0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x04, 0x02],
        "ML-KEM-1024" => vec![0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x04, 0x03],
        other => panic!("unknown ML-KEM variant: {other}"),
    }
}

fn mlkem_ek_len(variant: &str) -> usize {
    match variant {
        "ML-KEM-512" => 800,
        "ML-KEM-768" => 1184,
        "ML-KEM-1024" => 1568,
        other => panic!("unknown ML-KEM variant: {other}"),
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn find_last_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .rposition(|w| w == needle)
}

fn asn1_length(len: usize) -> Vec<u8> {
    if len < 128 {
        vec![len as u8]
    } else if len < 256 {
        vec![0x81, len as u8]
    } else if len < 65536 {
        vec![0x82, (len >> 8) as u8, (len & 0xFF) as u8]
    } else {
        vec![
            0x83,
            (len >> 16) as u8,
            ((len >> 8) & 0xFF) as u8,
            (len & 0xFF) as u8,
        ]
    }
}

fn asn1_sequence(content: &[u8]) -> Vec<u8> {
    let mut out = vec![0x30];
    out.extend_from_slice(&asn1_length(content.len()));
    out.extend_from_slice(content);
    out
}

fn asn1_octet_string(content: &[u8]) -> Vec<u8> {
    let mut out = vec![0x04];
    out.extend_from_slice(&asn1_length(content.len()));
    out.extend_from_slice(content);
    out
}

fn asn1_oid(oid_bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0x06];
    out.extend_from_slice(&asn1_length(oid_bytes.len()));
    out.extend_from_slice(oid_bytes);
    out
}

fn asn1_bit_string(content: &[u8]) -> Vec<u8> {
    let mut out = vec![0x03];
    out.extend_from_slice(&asn1_length(content.len() + 1)); // +1 for unused bits byte
    out.push(0x00); // no unused bits
    out.extend_from_slice(content);
    out
}
