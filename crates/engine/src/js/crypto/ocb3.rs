use aes::cipher::KeyInit;

type Block = aes::Block;

fn encrypt_block(cipher: &dyn AesBlockCipher, block: &mut Block) {
    // Dynamic dispatch over AES key sizes
    cipher.enc(block);
}

fn decrypt_block(cipher: &dyn AesBlockCipher, block: &mut Block) {
    cipher.dec(block);
}

trait AesBlockCipher {
    fn enc(&self, block: &mut Block);
    fn dec(&self, block: &mut Block);
}

impl AesBlockCipher for aes::Aes128 {
    fn enc(&self, block: &mut Block) {
        aes::cipher::BlockEncrypt::encrypt_block(self, block);
    }
    fn dec(&self, block: &mut Block) {
        aes::cipher::BlockDecrypt::decrypt_block(self, block);
    }
}

impl AesBlockCipher for aes::Aes192 {
    fn enc(&self, block: &mut Block) {
        aes::cipher::BlockEncrypt::encrypt_block(self, block);
    }
    fn dec(&self, block: &mut Block) {
        aes::cipher::BlockDecrypt::decrypt_block(self, block);
    }
}

impl AesBlockCipher for aes::Aes256 {
    fn enc(&self, block: &mut Block) {
        aes::cipher::BlockEncrypt::encrypt_block(self, block);
    }
    fn dec(&self, block: &mut Block) {
        aes::cipher::BlockDecrypt::decrypt_block(self, block);
    }
}

fn double(block: &[u8; 16]) -> [u8; 16] {
    let mut result = [0u8; 16];
    let carry = block[0] >> 7;
    for i in 0..15 {
        result[i] = (block[i] << 1) | (block[i + 1] >> 7);
    }
    result[15] = (block[15] << 1) ^ (carry * 0x87);
    result
}

fn xor_block(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i] ^ b[i];
    }
    r
}

fn ntz(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    n.trailing_zeros() as usize
}

struct Ocb3State {
    l_star: [u8; 16],
    l_dollar: [u8; 16],
    l: Vec<[u8; 16]>,
}

impl Ocb3State {
    fn new(cipher: &dyn AesBlockCipher) -> Self {
        let mut b = Block::default();
        encrypt_block(cipher, &mut b);
        let l_star: [u8; 16] = b.into();
        let l_dollar = double(&l_star);
        let mut l = vec![double(&l_dollar)];
        for _ in 1..32 {
            l.push(double(l.last().unwrap()));
        }
        Self {
            l_star,
            l_dollar,
            l,
        }
    }

    fn l_at(&self, idx: usize) -> &[u8; 16] {
        &self.l[idx.min(self.l.len() - 1)]
    }

    fn init_offset(&self, cipher: &dyn AesBlockCipher, nonce: &[u8], tag_bytes: usize) -> [u8; 16] {
        let tag_bits = tag_bytes * 8;
        let mut np = [0u8; 16];
        np[0] = ((tag_bits % 128) as u8) << 1;
        np[16 - nonce.len() - 1] |= 1;
        np[16 - nonce.len()..].copy_from_slice(nonce);

        let bottom = (np[15] & 0x3f) as usize;
        np[15] &= 0xc0;

        let mut b = Block::from(np);
        encrypt_block(cipher, &mut b);
        let ktop: [u8; 16] = b.into();

        let mut stretch = [0u8; 24];
        stretch[..16].copy_from_slice(&ktop);
        for i in 0..8 {
            stretch[16 + i] = ktop[i] ^ ktop[i + 1];
        }

        let mut offset = [0u8; 16];
        let bs = bottom / 8;
        let bi = bottom % 8;
        for i in 0..16 {
            offset[i] =
                (stretch[bs + i] << bi) | if bi > 0 { stretch[bs + i + 1] >> (8 - bi) } else { 0 };
        }
        offset
    }

    fn hash(&self, cipher: &dyn AesBlockCipher, aad: &[u8]) -> [u8; 16] {
        let mut sum = [0u8; 16];
        let mut offset = [0u8; 16];
        let full = aad.len() / 16;

        for i in 0..full {
            offset = xor_block(&offset, self.l_at(ntz(i + 1)));
            let mut block = [0u8; 16];
            block.copy_from_slice(&aad[i * 16..(i + 1) * 16]);
            let tmp = xor_block(&block, &offset);
            let mut b = Block::from(tmp);
            encrypt_block(cipher, &mut b);
            sum = xor_block(&sum, &<[u8; 16]>::from(b));
        }

        let rem = aad.len() % 16;
        if rem > 0 {
            offset = xor_block(&offset, &self.l_star);
            let mut block = [0u8; 16];
            block[..rem].copy_from_slice(&aad[full * 16..]);
            block[rem] = 0x80;
            let tmp = xor_block(&block, &offset);
            let mut b = Block::from(tmp);
            encrypt_block(cipher, &mut b);
            sum = xor_block(&sum, &<[u8; 16]>::from(b));
        }

        sum
    }
}

fn ocb3_encrypt_core(
    cipher: &dyn AesBlockCipher,
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
    tag_bytes: usize,
) -> Vec<u8> {
    let state = Ocb3State::new(cipher);
    let mut offset = state.init_offset(cipher, nonce, tag_bytes);
    let full = plaintext.len() / 16;
    let mut ct = vec![0u8; plaintext.len()];
    let mut checksum = [0u8; 16];

    for i in 0..full {
        offset = xor_block(&offset, state.l_at(ntz(i + 1)));
        let mut block = [0u8; 16];
        block.copy_from_slice(&plaintext[i * 16..(i + 1) * 16]);
        checksum = xor_block(&checksum, &block);
        let tmp = xor_block(&block, &offset);
        let mut b = Block::from(tmp);
        encrypt_block(cipher, &mut b);
        let ct_block = xor_block(&<[u8; 16]>::from(b), &offset);
        ct[i * 16..(i + 1) * 16].copy_from_slice(&ct_block);
    }

    let rem = plaintext.len() % 16;
    if rem > 0 {
        offset = xor_block(&offset, &state.l_star);
        let mut pad = Block::from(offset);
        encrypt_block(cipher, &mut pad);
        let pad_bytes: [u8; 16] = pad.into();
        let start = full * 16;
        for i in 0..rem {
            ct[start + i] = plaintext[start + i] ^ pad_bytes[i];
        }
        let mut last = [0u8; 16];
        last[..rem].copy_from_slice(&plaintext[start..]);
        last[rem] = 0x80;
        checksum = xor_block(&checksum, &last);
    }

    let tag_in = xor_block(&xor_block(&checksum, &offset), &state.l_dollar);
    let mut b = Block::from(tag_in);
    encrypt_block(cipher, &mut b);
    let tag_enc: [u8; 16] = b.into();
    let hash_val = state.hash(cipher, aad);
    let full_tag = xor_block(&tag_enc, &hash_val);

    ct.extend_from_slice(&full_tag[..tag_bytes]);
    ct
}

fn ocb3_decrypt_core(
    cipher: &dyn AesBlockCipher,
    nonce: &[u8],
    aad: &[u8],
    ciphertext_with_tag: &[u8],
    tag_bytes: usize,
) -> Option<Vec<u8>> {
    if ciphertext_with_tag.len() < tag_bytes {
        return None;
    }
    let ct_len = ciphertext_with_tag.len() - tag_bytes;
    let ct = &ciphertext_with_tag[..ct_len];
    let given_tag = &ciphertext_with_tag[ct_len..];

    let state = Ocb3State::new(cipher);
    let mut offset = state.init_offset(cipher, nonce, tag_bytes);
    let full = ct.len() / 16;
    let mut pt = vec![0u8; ct.len()];
    let mut checksum = [0u8; 16];

    for i in 0..full {
        offset = xor_block(&offset, state.l_at(ntz(i + 1)));
        let mut block = [0u8; 16];
        block.copy_from_slice(&ct[i * 16..(i + 1) * 16]);
        let tmp = xor_block(&block, &offset);
        let mut b = Block::from(tmp);
        decrypt_block(cipher, &mut b);
        let pt_block = xor_block(&<[u8; 16]>::from(b), &offset);
        pt[i * 16..(i + 1) * 16].copy_from_slice(&pt_block);
        checksum = xor_block(&checksum, &pt_block);
    }

    let rem = ct.len() % 16;
    if rem > 0 {
        offset = xor_block(&offset, &state.l_star);
        let mut pad = Block::from(offset);
        encrypt_block(cipher, &mut pad);
        let pad_bytes: [u8; 16] = pad.into();
        let start = full * 16;
        for i in 0..rem {
            pt[start + i] = ct[start + i] ^ pad_bytes[i];
        }
        let mut last = [0u8; 16];
        last[..rem].copy_from_slice(&pt[start..]);
        last[rem] = 0x80;
        checksum = xor_block(&checksum, &last);
    }

    let tag_in = xor_block(&xor_block(&checksum, &offset), &state.l_dollar);
    let mut b = Block::from(tag_in);
    encrypt_block(cipher, &mut b);
    let tag_enc: [u8; 16] = b.into();
    let hash_val = state.hash(cipher, aad);
    let full_tag = xor_block(&tag_enc, &hash_val);

    let mut diff = 0u8;
    for i in 0..tag_bytes {
        diff |= full_tag[i] ^ given_tag[i];
    }
    if diff != 0 {
        return None;
    }

    Some(pt)
}

pub fn ocb3_encrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
    tag_bytes: usize,
) -> Vec<u8> {
    match key.len() {
        16 => {
            let c = aes::Aes128::new_from_slice(key).unwrap();
            ocb3_encrypt_core(&c, nonce, aad, plaintext, tag_bytes)
        }
        24 => {
            let c = aes::Aes192::new_from_slice(key).unwrap();
            ocb3_encrypt_core(&c, nonce, aad, plaintext, tag_bytes)
        }
        32 => {
            let c = aes::Aes256::new_from_slice(key).unwrap();
            ocb3_encrypt_core(&c, nonce, aad, plaintext, tag_bytes)
        }
        _ => panic!("OperationError: AES-OCB key must be 128, 192, or 256 bits"),
    }
}

pub fn ocb3_decrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    ciphertext_with_tag: &[u8],
    tag_bytes: usize,
) -> Option<Vec<u8>> {
    match key.len() {
        16 => {
            let c = aes::Aes128::new_from_slice(key).unwrap();
            ocb3_decrypt_core(&c, nonce, aad, ciphertext_with_tag, tag_bytes)
        }
        24 => {
            let c = aes::Aes192::new_from_slice(key).unwrap();
            ocb3_decrypt_core(&c, nonce, aad, ciphertext_with_tag, tag_bytes)
        }
        32 => {
            let c = aes::Aes256::new_from_slice(key).unwrap();
            ocb3_decrypt_core(&c, nonce, aad, ciphertext_with_tag, tag_bytes)
        }
        _ => panic!("OperationError: AES-OCB key must be 128, 192, or 256 bits"),
    }
}
