use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_chacha20_encrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, plaintext: Vec<u8>, aad: Vec<u8>| -> Vec<u8> {
                let cipher = ChaCha20Poly1305::new_from_slice(&key).expect("bad key");
                let nonce = Nonce::from_slice(&iv);
                let mut buffer = plaintext;
                let tag = cipher
                    .encrypt_in_place_detached(nonce, &aad, &mut buffer)
                    .expect("ChaCha20-Poly1305 encrypt failed");
                buffer.extend_from_slice(&tag);
                buffer
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_chacha20_decrypt",
        Function::new(
            ctx.clone(),
            |key: Vec<u8>, iv: Vec<u8>, ciphertext: Vec<u8>, aad: Vec<u8>| -> Vec<Vec<u8>> {
                if ciphertext.len() < 16 {
                    return vec![vec![0]];
                }
                let ct_len = ciphertext.len() - 16;
                let ct = &ciphertext[..ct_len];
                let tag = chacha20poly1305::Tag::from_slice(&ciphertext[ct_len..]);
                let cipher = ChaCha20Poly1305::new_from_slice(&key).expect("bad key");
                let nonce = Nonce::from_slice(&iv);
                let mut buffer = ct.to_vec();
                match cipher.decrypt_in_place_detached(nonce, &aad, &mut buffer, tag) {
                    Ok(()) => vec![vec![1], buffer],
                    Err(_) => vec![vec![0]],
                }
            },
        )
        .unwrap(),
    )
    .unwrap();
}
