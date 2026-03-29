use rquickjs::{Ctx, Function};

pub fn register(ctx: &Ctx<'_>) {
    let g = ctx.globals();

    g.set(
        "__braille_crypto_get_random_bytes",
        Function::new(ctx.clone(), |len: u32| -> Vec<u8> {
            let mut buf = vec![0u8; len as usize];
            getrandom::getrandom(&mut buf).expect("getrandom failed");
            buf
        })
        .unwrap(),
    )
    .unwrap();
}
