//! Shared base64url encode/decode utilities for WebCrypto JWK operations.

const B64_TABLE: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn b64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_TABLE[(triple >> 18 & 0x3F) as usize] as char);
        out.push(B64_TABLE[(triple >> 12 & 0x3F) as usize] as char);
        if i + 1 < data.len() {
            out.push(B64_TABLE[(triple >> 6 & 0x3F) as usize] as char);
        }
        if i + 2 < data.len() {
            out.push(B64_TABLE[(triple & 0x3F) as usize] as char);
        }
        i += 3;
    }
    out
}

pub fn b64url_decode(input: &str) -> Vec<u8> {
    // Reverse lookup table: ASCII byte → 6-bit value (255 = invalid)
    const DECODE: [u8; 128] = {
        let mut t = [255u8; 128];
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < 64 {
            t[alphabet[i] as usize] = i as u8;
            i += 1;
        }
        // base64url alternatives
        t[b'-' as usize] = 62;
        t[b'_' as usize] = 63;
        t
    };

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut i = 0;
    let len = bytes.len();
    while i < len {
        let b0 = bytes.get(i).copied().unwrap_or(b'=');
        let b1 = bytes.get(i + 1).copied().unwrap_or(b'=');
        let b2 = bytes.get(i + 2).copied().unwrap_or(b'=');
        let b3 = bytes.get(i + 3).copied().unwrap_or(b'=');
        if b0 == b'=' {
            break;
        }
        let v0 = if (b0 as usize) < 128 { DECODE[b0 as usize] } else { 0 } as u32;
        let v1 = if b1 != b'=' && (b1 as usize) < 128 { DECODE[b1 as usize] } else { 0 } as u32;
        let v2 = if b2 != b'=' && (b2 as usize) < 128 { DECODE[b2 as usize] } else { 0 } as u32;
        let v3 = if b3 != b'=' && (b3 as usize) < 128 { DECODE[b3 as usize] } else { 0 } as u32;
        let triple = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push((triple >> 16) as u8);
        if b2 != b'=' {
            out.push((triple >> 8 & 0xFF) as u8);
        }
        if b3 != b'=' {
            out.push((triple & 0xFF) as u8);
        }
        i += 4;
    }
    out
}

/// Normalize base64url input to standard base64 with padding, then decode.
pub fn b64url_decode_url(input: &str) -> Vec<u8> {
    let s = input.replace('-', "+").replace('_', "/");
    let padded = match s.len() % 4 {
        2 => format!("{s}=="),
        3 => format!("{s}="),
        _ => s,
    };
    b64url_decode(&padded)
}
