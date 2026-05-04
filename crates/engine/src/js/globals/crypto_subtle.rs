use rquickjs::Ctx;

pub(crate) fn register_crypto(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(crypto_js()).unwrap();
}

fn crypto_js() -> &'static str {
    concat!(
        "(function() {\n",
        include_str!("crypto_js/helpers.js"),
        "\n",
        "    // ---- subtle ----\n",
        "    var subtle = {\n",
        include_str!("crypto_js/digest.js"),
        "\n",
        include_str!("crypto_js/generate_key.js"),
        "\n",
        include_str!("crypto_js/import_key.js"),
        "\n",
        include_str!("crypto_js/export_key.js"),
        "\n",
        include_str!("crypto_js/encrypt_decrypt.js"),
        "\n",
        include_str!("crypto_js/sign_verify.js"),
        "\n",
        include_str!("crypto_js/derive.js"),
        "\n",
        include_str!("crypto_js/kem.js"),
        "\n",
        "    };\n",
        "\n",
        include_str!("crypto_js/random.js"),
        "\n",
        include_str!("crypto_js/supports.js"),
        "\n",
        "})();\n"
    )
}
