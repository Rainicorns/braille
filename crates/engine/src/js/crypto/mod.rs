mod digest;
mod aes;
mod chacha;
mod ec;
mod hmac;
mod kdf;
mod mlkem;
mod ocb3;
mod random;
mod rsa;

use rquickjs::Ctx;

pub fn register(ctx: &Ctx<'_>) {
    random::register(ctx);
    digest::register(ctx);
    aes::register(ctx);
    hmac::register(ctx);
    kdf::register(ctx);
    ec::register(ctx);
    mlkem::register(ctx);
    chacha::register(ctx);
    rsa::register(ctx);
}
