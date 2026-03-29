use aes_gcm::aead::consts::{U12, U32};
use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::AesGcm;
use rquickjs::{Ctx, Function};

macro_rules! gcm_encrypt {
    ($aes:ty, $nonce_size:ty, $key:expr, $iv:expr, $pt:expr, $aad:expr, $tag_bytes:expr) => {{
        let cipher = AesGcm::<$aes, $nonce_size>::new_from_slice($key).expect("bad key");
        let nonce = aes_gcm::Nonce::<$nonce_size>::from_slice($iv);
        let mut buffer = $pt.to_vec();
        let tag = cipher
            .encrypt_in_place_detached(nonce, $aad, &mut buffer)
            .expect("AES-GCM encrypt failed");
        buffer.extend_from_slice(&tag[..$tag_bytes]);
        buffer
    }};
}

macro_rules! gcm_decrypt {
    ($aes:ty, $nonce_size:ty, $key:expr, $iv:expr, $ct:expr, $truncated_tag:expr, $aad:expr, $tag_bytes:expr) => {{
        let cipher = AesGcm::<$aes, $nonce_size>::new_from_slice($key).expect("bad key");
        let nonce = aes_gcm::Nonce::<$nonce_size>::from_slice($iv);
        // Get plaintext via double-encrypt trick (CTR is self-inverse):
        // 1. encrypt_in_place(ct) -> pt (CTR decrypts), but tag is wrong (computed over pt not ct)
        // 2. encrypt_in_place(pt) -> ct + correct_tag (computed over ct)
        let mut pt_buf = $ct.to_vec();
        let _wrong_tag = cipher
            .encrypt_in_place_detached(nonce, $aad, &mut pt_buf)
            .expect("decrypt step 1 failed");
        // pt_buf now has plaintext. Re-encrypt to get the correct tag.
        let mut verify_buf = pt_buf.clone();
        let correct_tag = cipher
            .encrypt_in_place_detached(nonce, $aad, &mut verify_buf)
            .expect("decrypt step 2 failed");
        if correct_tag[..$tag_bytes] == *$truncated_tag {
            vec![vec![1], pt_buf]
        } else {
            vec![vec![0]]
        }
    }};
}

fn aes_gcm_encrypt_impl(
    key: &[u8],
    iv: &[u8],
    plaintext: &[u8],
    aad: &[u8],
    tag_bits: u32,
) -> Vec<u8> {
    let tag_bytes = (tag_bits / 8) as usize;

    match (key.len(), iv.len()) {
        (16, 12) => gcm_encrypt!(aes::Aes128, U12, key, iv, plaintext, aad, tag_bytes),
        (24, 12) => gcm_encrypt!(aes::Aes192, U12, key, iv, plaintext, aad, tag_bytes),
        (32, 12) => gcm_encrypt!(aes::Aes256, U12, key, iv, plaintext, aad, tag_bytes),
        (16, 32) => gcm_encrypt!(aes::Aes128, U32, key, iv, plaintext, aad, tag_bytes),
        (24, 32) => gcm_encrypt!(aes::Aes192, U32, key, iv, plaintext, aad, tag_bytes),
        (32, 32) => gcm_encrypt!(aes::Aes256, U32, key, iv, plaintext, aad, tag_bytes),
        _ => panic!(
            "OperationError: unsupported AES-GCM key size {} or IV size {}",
            key.len(),
            iv.len()
        ),
    }
}

fn aes_gcm_decrypt_impl(
    key: &[u8],
    iv: &[u8],
    ciphertext_with_tag: &[u8],
    aad: &[u8],
    tag_bits: u32,
) -> Vec<Vec<u8>> {
    let tag_bytes = (tag_bits / 8) as usize;
    if ciphertext_with_tag.len() < tag_bytes {
        return vec![vec![0]];
    }

    let ct_len = ciphertext_with_tag.len() - tag_bytes;
    let ct = &ciphertext_with_tag[..ct_len];
    let truncated_tag = &ciphertext_with_tag[ct_len..];

    match (key.len(), iv.len()) {
        (16, 12) => gcm_decrypt!(aes::Aes128, U12, key, iv, ct, truncated_tag, aad, tag_bytes),
        (24, 12) => gcm_decrypt!(aes::Aes192, U12, key, iv, ct, truncated_tag, aad, tag_bytes),
        (32, 12) => gcm_decrypt!(aes::Aes256, U12, key, iv, ct, truncated_tag, aad, tag_bytes),
        (16, 32) => gcm_decrypt!(aes::Aes128, U32, key, iv, ct, truncated_tag, aad, tag_bytes),
        (24, 32) => gcm_decrypt!(aes::Aes192, U32, key, iv, ct, truncated_tag, aad, tag_bytes),
        (32, 32) => gcm_decrypt!(aes::Aes256, U32, key, iv, ct, truncated_tag, aad, tag_bytes),
        _ => panic!(
            "OperationError: unsupported AES-GCM key size {} or IV size {}",
            key.len(),
            iv.len()
        ),
    }
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_aes_gcm_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>,
             iv: Vec<u8>,
             plaintext: Vec<u8>,
             aad: Vec<u8>,
             tag_bits: u32|
             -> Vec<u8> { aes_gcm_encrypt_impl(&key, &iv, &plaintext, &aad, tag_bits) },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_gcm_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>,
             iv: Vec<u8>,
             ciphertext: Vec<u8>,
             aad: Vec<u8>,
             tag_bits: u32|
             -> Vec<Vec<u8>> {
                aes_gcm_decrypt_impl(&key, &iv, &ciphertext, &aad, tag_bits)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_cbc_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, plaintext: Vec<u8>| -> Vec<u8> {
                use aes::cipher::{BlockEncryptMut, KeyIvInit};
                type Pad = aes::cipher::block_padding::Pkcs7;

                match key.len() {
                    16 => {
                        let enc = cbc::Encryptor::<aes::Aes128>::new(
                            key.as_slice().into(),
                            iv.as_slice().into(),
                        );
                        enc.encrypt_padded_vec_mut::<Pad>(&plaintext)
                    }
                    24 => {
                        let enc = cbc::Encryptor::<aes::Aes192>::new(
                            key.as_slice().into(),
                            iv.as_slice().into(),
                        );
                        enc.encrypt_padded_vec_mut::<Pad>(&plaintext)
                    }
                    32 => {
                        let enc = cbc::Encryptor::<aes::Aes256>::new(
                            key.as_slice().into(),
                            iv.as_slice().into(),
                        );
                        enc.encrypt_padded_vec_mut::<Pad>(&plaintext)
                    }
                    _ => panic!("OperationError: AES-CBC key must be 128, 192, or 256 bits"),
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_cbc_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, ciphertext: Vec<u8>| -> Vec<Vec<u8>> {
                use aes::cipher::{BlockDecryptMut, KeyIvInit};
                type Pad = aes::cipher::block_padding::Pkcs7;

                let result = match key.len() {
                    16 => {
                        let dec = cbc::Decryptor::<aes::Aes128>::new(
                            key.as_slice().into(),
                            iv.as_slice().into(),
                        );
                        dec.decrypt_padded_vec_mut::<Pad>(&ciphertext)
                    }
                    24 => {
                        let dec = cbc::Decryptor::<aes::Aes192>::new(
                            key.as_slice().into(),
                            iv.as_slice().into(),
                        );
                        dec.decrypt_padded_vec_mut::<Pad>(&ciphertext)
                    }
                    32 => {
                        let dec = cbc::Decryptor::<aes::Aes256>::new(
                            key.as_slice().into(),
                            iv.as_slice().into(),
                        );
                        dec.decrypt_padded_vec_mut::<Pad>(&ciphertext)
                    }
                    _ => panic!("OperationError: AES-CBC key must be 128, 192, or 256 bits"),
                };
                match result {
                    Ok(data) => vec![vec![1], data],
                    Err(_) => vec![vec![0]],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_ctr_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, counter: Vec<u8>, plaintext: Vec<u8>| -> Vec<u8> {
                use aes::cipher::{KeyIvInit, StreamCipher};

                let mut buf = plaintext;
                match key.len() {
                    16 => {
                        let mut cipher = ctr::Ctr128BE::<aes::Aes128>::new(
                            key.as_slice().into(),
                            counter.as_slice().into(),
                        );
                        cipher.apply_keystream(&mut buf);
                    }
                    24 => {
                        let mut cipher = ctr::Ctr128BE::<aes::Aes192>::new(
                            key.as_slice().into(),
                            counter.as_slice().into(),
                        );
                        cipher.apply_keystream(&mut buf);
                    }
                    32 => {
                        let mut cipher = ctr::Ctr128BE::<aes::Aes256>::new(
                            key.as_slice().into(),
                            counter.as_slice().into(),
                        );
                        cipher.apply_keystream(&mut buf);
                    }
                    _ => panic!("OperationError: AES-CTR key must be 128, 192, or 256 bits"),
                }
                buf
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_ctr_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, counter: Vec<u8>, ciphertext: Vec<u8>| -> Vec<u8> {
                use aes::cipher::{KeyIvInit, StreamCipher};

                let mut buf = ciphertext;
                match key.len() {
                    16 => {
                        let mut cipher = ctr::Ctr128BE::<aes::Aes128>::new(
                            key.as_slice().into(),
                            counter.as_slice().into(),
                        );
                        cipher.apply_keystream(&mut buf);
                    }
                    24 => {
                        let mut cipher = ctr::Ctr128BE::<aes::Aes192>::new(
                            key.as_slice().into(),
                            counter.as_slice().into(),
                        );
                        cipher.apply_keystream(&mut buf);
                    }
                    32 => {
                        let mut cipher = ctr::Ctr128BE::<aes::Aes256>::new(
                            key.as_slice().into(),
                            counter.as_slice().into(),
                        );
                        cipher.apply_keystream(&mut buf);
                    }
                    _ => panic!("OperationError: AES-CTR key must be 128, 192, or 256 bits"),
                }
                buf
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_ocb_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>,
             iv: Vec<u8>,
             plaintext: Vec<u8>,
             aad: Vec<u8>,
             tag_bits: u32|
             -> Vec<u8> {
                super::ocb3::ocb3_encrypt(&key, &iv, &aad, &plaintext, (tag_bits / 8) as usize)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_aes_ocb_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>,
             iv: Vec<u8>,
             ciphertext: Vec<u8>,
             aad: Vec<u8>,
             tag_bits: u32|
             -> Vec<Vec<u8>> {
                match super::ocb3::ocb3_decrypt(
                    &key,
                    &iv,
                    &aad,
                    &ciphertext,
                    (tag_bits / 8) as usize,
                ) {
                    Some(pt) => vec![vec![1], pt],
                    None => vec![vec![0]],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();
}
