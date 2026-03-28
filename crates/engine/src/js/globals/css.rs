use rquickjs::Ctx;

pub(super) fn register_css_object(ctx: &Ctx<'_>) {
    // CSS global with supports() backed by native __n_cssSupports, plus CSS.escape()
    ctx.eval::<(), _>(r#"
        globalThis.CSS = {
            supports: function(propOrCondition, value) {
                if (arguments.length >= 2) {
                    return __n_cssSupports(String(propOrCondition) + ': ' + String(value));
                }
                var cond = String(propOrCondition).trim();
                // Strip outer parens: "(display: flex)" -> "display: flex"
                if (cond.charAt(0) === '(' && cond.charAt(cond.length - 1) === ')') {
                    cond = cond.substring(1, cond.length - 1).trim();
                }
                return __n_cssSupports(cond);
            },
            escape: function(value) {
                var s = String(value);
                var result = '';
                for (var i = 0; i < s.length; i++) {
                    var ch = s.charCodeAt(i);
                    if (ch === 0) { result += '\uFFFD'; continue; }
                    if ((ch >= 1 && ch <= 31) || ch === 127) { result += '\\' + ch.toString(16) + ' '; continue; }
                    if (i === 0 && ch >= 48 && ch <= 57) { result += '\\' + ch.toString(16) + ' '; continue; }
                    if (i === 0 && ch === 45 && s.length === 1) { result += '\\-'; continue; }
                    if (ch === 45 || ch === 95 || (ch >= 48 && ch <= 57) || (ch >= 65 && ch <= 90) || (ch >= 97 && ch <= 122) || ch >= 128) {
                        result += s.charAt(i); continue;
                    }
                    result += '\\' + s.charAt(i);
                }
                return result;
            }
        };
    "#).unwrap();
}
