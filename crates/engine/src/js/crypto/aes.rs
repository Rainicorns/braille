use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

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

    g.set(
        "__braille_crypto_aes_cbc_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, plaintext: Vec<u8>| -> Vec<u8> {
                use aes::cipher::{BlockEncryptMut, KeyIvInit};

                match key.len() {
                    16 => {
                        type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
                        let encryptor = Aes128CbcEnc::new(key.as_slice().into(), iv.as_slice().into());
                        encryptor.encrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&plaintext)
                    }
                    32 => {
                        type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
                        let encryptor = Aes256CbcEnc::new(key.as_slice().into(), iv.as_slice().into());
                        encryptor.encrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&plaintext)
                    }
                    _ => panic!("OperationError: AES-CBC key must be 128 or 256 bits"),
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
            |key: Vec<u8>, iv: Vec<u8>, ciphertext: Vec<u8>| -> Vec<u8> {
                use aes::cipher::{BlockDecryptMut, KeyIvInit};

                match key.len() {
                    16 => {
                        type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
                        let decryptor = Aes128CbcDec::new(key.as_slice().into(), iv.as_slice().into());
                        decryptor
                            .decrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&ciphertext)
                            .expect("OperationError: AES-CBC decryption failed")
                    }
                    32 => {
                        type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
                        let decryptor = Aes256CbcDec::new(key.as_slice().into(), iv.as_slice().into());
                        decryptor
                            .decrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(&ciphertext)
                            .expect("OperationError: AES-CBC decryption failed")
                    }
                    _ => panic!("OperationError: AES-CBC key must be 128 or 256 bits"),
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
                        type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;
                        let mut cipher = Aes128Ctr::new(key.as_slice().into(), counter.as_slice().into());
                        cipher.apply_keystream(&mut buf);
                    }
                    32 => {
                        type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;
                        let mut cipher = Aes256Ctr::new(key.as_slice().into(), counter.as_slice().into());
                        cipher.apply_keystream(&mut buf);
                    }
                    _ => panic!("OperationError: AES-CTR key must be 128 or 256 bits"),
                }
                buf
            },
        )
        .unwrap(),
    )
    .unwrap();

    // AES-CTR decrypt is the same operation as encrypt (XOR cipher)
    g.set(
        "__braille_crypto_aes_ctr_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, counter: Vec<u8>, ciphertext: Vec<u8>| -> Vec<u8> {
                use aes::cipher::{KeyIvInit, StreamCipher};

                let mut buf = ciphertext;
                match key.len() {
                    16 => {
                        type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;
                        let mut cipher = Aes128Ctr::new(key.as_slice().into(), counter.as_slice().into());
                        cipher.apply_keystream(&mut buf);
                    }
                    32 => {
                        type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;
                        let mut cipher = Aes256Ctr::new(key.as_slice().into(), counter.as_slice().into());
                        cipher.apply_keystream(&mut buf);
                    }
                    _ => panic!("OperationError: AES-CTR key must be 128 or 256 bits"),
                }
                buf
            },
        )
        .unwrap(),
    )
    .unwrap();
}
