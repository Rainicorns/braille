use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{native_function::NativeFunction, Context, JsResult, JsValue};

pub(crate) type ConsoleBuffer = Rc<RefCell<Vec<String>>>;

pub(super) fn console_format_args(args: &[JsValue], ctx: &mut Context) -> JsResult<String> {
    let parts: Vec<String> = args
        .iter()
        .map(|v| v.to_string(ctx).map(|s| s.to_std_string_escaped()))
        .collect::<JsResult<Vec<String>>>()?;
    Ok(parts.join(" "))
}

pub(super) fn make_console_method(buffer: ConsoleBuffer, prefix: Option<&'static str>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let msg = console_format_args(args, ctx)?;
            let formatted = match prefix {
                Some(p) => format!("{}{}", p, msg),
                None => msg,
            };
            buffer.borrow_mut().push(formatted);
            Ok(JsValue::undefined())
        })
    }
}
