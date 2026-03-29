use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    // PBKDF2
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

    // HKDF
    g.set(
        "__braille_crypto_hkdf",
        Function::new(
            ctx.clone(),
            |algo: String,
             key_material: Vec<u8>,
             salt: Vec<u8>,
             info: Vec<u8>,
             output_len: u32|
             -> Vec<u8> {
                let algorithm = match algo.as_str() {
                    "SHA-1" => ring::hkdf::HKDF_SHA1_FOR_LEGACY_USE_ONLY,
                    "SHA-256" => ring::hkdf::HKDF_SHA256,
                    "SHA-384" => ring::hkdf::HKDF_SHA384,
                    "SHA-512" => ring::hkdf::HKDF_SHA512,
                    other => panic!("NotSupportedError: HKDF hash '{other}' not supported"),
                };
                let salt = ring::hkdf::Salt::new(algorithm, &salt);
                let prk = salt.extract(&key_material);
                let info_refs: &[&[u8]] = &[&info];
                let okm = prk
                    .expand(info_refs, HkdfLen(output_len as usize))
                    .expect("OperationError: HKDF expand failed");
                let mut out = vec![0u8; output_len as usize];
                okm.fill(&mut out)
                    .expect("OperationError: HKDF fill failed");
                out
            },
        )
        .unwrap(),
    )
    .unwrap();

    // Argon2: params = [memory, passes, parallelism, output_len]
    g.set(
        "__braille_crypto_argon2",
        Function::new(
            ctx.clone(),
            |variant: String,
             password: Vec<u8>,
             salt: Vec<u8>,
             params_arr: Vec<u32>,
             secret: Vec<u8>,
             ad: Vec<u8>|
             -> Vec<u8> {
                use argon2::{Algorithm, Argon2, AssociatedData, ParamsBuilder, Version};

                let memory = params_arr[0];
                let passes = params_arr[1];
                let parallelism = params_arr[2];
                let output_len = params_arr[3];

                let algorithm = match variant.as_str() {
                    "d" => Algorithm::Argon2d,
                    "i" => Algorithm::Argon2i,
                    "id" => Algorithm::Argon2id,
                    other => panic!("NotSupportedError: Argon2 variant '{other}' not supported"),
                };

                let mut builder = ParamsBuilder::new();
                builder.m_cost(memory);
                builder.t_cost(passes);
                builder.p_cost(parallelism);
                builder.output_len(output_len as usize);
                if !ad.is_empty() {
                    builder.data(
                        AssociatedData::new(&ad)
                            .expect("OperationError: associated data too long"),
                    );
                }
                let params = builder
                    .build()
                    .expect("OperationError: invalid Argon2 parameters");

                let secret_ref = if secret.is_empty() {
                    &[]
                } else {
                    secret.as_slice()
                };

                let argon2 =
                    Argon2::new_with_secret(secret_ref, algorithm, Version::V0x13, params)
                        .expect("OperationError: failed to create Argon2 instance");

                let mut out = vec![0u8; output_len as usize];
                argon2
                    .hash_password_into(&password, &salt, &mut out)
                    .expect("OperationError: Argon2 hash failed");
                out
            },
        )
        .unwrap(),
    )
    .unwrap();
}

struct HkdfLen(usize);

impl ring::hkdf::KeyType for HkdfLen {
    fn len(&self) -> usize {
        self.0
    }
}
