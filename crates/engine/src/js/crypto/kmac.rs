use rquickjs::{Ctx, Function};
use sha3::digest::core_api::CoreWrapper;
use sha3::digest::{ExtendableOutput, Update, XofReader};

fn left_encode(x: usize) -> Vec<u8> {
    if x == 0 {
        return vec![1, 0];
    }
    let mut buf = Vec::new();
    let mut val = x;
    while val > 0 {
        buf.push((val & 0xff) as u8);
        val >>= 8;
    }
    buf.reverse();
    let len = buf.len() as u8;
    let mut result = vec![len];
    result.extend_from_slice(&buf);
    result
}

fn right_encode(x: usize) -> Vec<u8> {
    if x == 0 {
        return vec![0, 1];
    }
    let mut buf = Vec::new();
    let mut val = x;
    while val > 0 {
        buf.push((val & 0xff) as u8);
        val >>= 8;
    }
    buf.reverse();
    let len = buf.len() as u8;
    buf.push(len);
    buf
}

fn encode_string(s: &[u8]) -> Vec<u8> {
    let mut result = left_encode(s.len() * 8);
    result.extend_from_slice(s);
    result
}

fn bytepad(x: &[u8], w: usize) -> Vec<u8> {
    let mut result = left_encode(w);
    result.extend_from_slice(x);
    let pad_len = (w - (result.len() % w)) % w;
    result.resize(result.len() + pad_len, 0);
    result
}

// KMAC per NIST SP 800-185
// KMAC(K, X, L, S) = cSHAKE(bytepad(encode_string(K), rate) || X || right_encode(L), L, "KMAC", S)
fn kmac(algo: &str, key: &[u8], data: &[u8], length_bits: usize, customization: &[u8]) -> Vec<u8> {
    let rate = if algo == "KMAC128" { 168 } else { 136 };
    let padded_key = bytepad(&encode_string(key), rate);
    let output_bytes = length_bits / 8;

    if algo == "KMAC128" {
        let core = sha3::CShake128Core::new_with_function_name(b"KMAC", customization);
        let mut hasher: CoreWrapper<sha3::CShake128Core> = CoreWrapper::from_core(core);
        hasher.update(&padded_key);
        hasher.update(data);
        hasher.update(&right_encode(length_bits));
        let mut output = vec![0u8; output_bytes];
        hasher.finalize_xof().read(&mut output);
        output
    } else {
        let core = sha3::CShake256Core::new_with_function_name(b"KMAC", customization);
        let mut hasher: CoreWrapper<sha3::CShake256Core> = CoreWrapper::from_core(core);
        hasher.update(&padded_key);
        hasher.update(data);
        hasher.update(&right_encode(length_bits));
        let mut output = vec![0u8; output_bytes];
        hasher.finalize_xof().read(&mut output);
        output
    }
}

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_kmac_sign",
        Function::new(
            ctx.clone(),
            |algo: String,
             key: Vec<u8>,
             data: Vec<u8>,
             length_bits: usize,
             customization: Vec<u8>|
             -> Vec<u8> {
                kmac(&algo, &key, &data, length_bits, &customization)
            },
        )
        .unwrap(),
    )
    .unwrap();

    g.set(
        "__braille_crypto_kmac_verify",
        Function::new(
            ctx.clone(),
            |algo: String,
             key: Vec<u8>,
             signature: Vec<u8>,
             data: Vec<u8>,
             length_bits: usize,
             customization: Vec<u8>|
             -> bool {
                let expected = kmac(&algo, &key, &data, length_bits, &customization);
                expected == signature
            },
        )
        .unwrap(),
    )
    .unwrap();
}
