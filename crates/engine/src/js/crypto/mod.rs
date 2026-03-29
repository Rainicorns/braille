mod digest;
mod aes;
mod ec;
mod hmac;
mod kdf;
mod random;

use rquickjs::Ctx;

pub fn register(ctx: &Ctx<'_>) {
    random::register(ctx);
    digest::register(ctx);
    aes::register(ctx);
    hmac::register(ctx);
    kdf::register(ctx);
    ec::register(ctx);
}
