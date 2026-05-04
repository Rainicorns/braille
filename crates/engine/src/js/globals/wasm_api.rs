use rquickjs::Ctx;

pub(crate) fn register_wasm(ctx: &Ctx<'_>) {
    ctx.eval::<(), _>(wasm_js()).unwrap();
}

fn wasm_js() -> &'static str {
    concat!(
        "(function() {\n",
        include_str!("wasm_js/errors.js"),
        "\n",
        include_str!("wasm_js/memory.js"),
        "\n",
        include_str!("wasm_js/table.js"),
        "\n",
        include_str!("wasm_js/global.js"),
        "\n",
        include_str!("wasm_js/module.js"),
        "\n",
        include_str!("wasm_js/instance.js"),
        "\n",
        include_str!("wasm_js/namespace.js"),
        "\n",
        "})();\n"
    )
}
