# Fix: `:invalid` Pseudo-Class for Element-closest

## Current Score: Element-closest 28/29 (target: 29/29)

## Problem

One test case at line 57 of Element-closest.html fails:
```javascript
do_test(":invalid", "test11", "test2");
```
Starting from `test11` (an `<option selected>`), `.closest(":invalid")` should return `test2` (a `<fieldset>` containing a required empty input).

## Why test2 is :invalid

```html
<fieldset id="test2">
  <select id="test3" required>
    <option id="test11" selected>Test11</option>  <!-- start here -->
  </select>
  <input id="test9" type="text" required>  <!-- empty + required = invalid -->
</fieldset>
```

- `test9` is `<input type="text" required>` with no value → `:invalid`
- `test2` is `<fieldset>` containing an invalid descendant → `:invalid`

## Implementation (~50-60 lines)

### File 1: `crates/engine/src/css/selector_impl.rs`

**1a. Add variants to PseudoClass enum (line ~79):**
```
Invalid,
Valid,
```

**1b. Add ToCss arms:**
```
PseudoClass::Invalid => dest.write_str(":invalid"),
PseudoClass::Valid => dest.write_str(":valid"),
```

**1c. Add parser cases in `parse_non_ts_pseudo_class`:**
```
"invalid" => Ok(PseudoClass::Invalid),
"valid" => Ok(PseudoClass::Valid),
```

### File 2: `crates/engine/src/css/matching.rs`

**2a. Add match arms in `match_non_ts_pseudo_class`:**

For `PseudoClass::Invalid`:
- If tag is `input`/`textarea`: check `required` attr + empty value → return true
- If tag is `select`: check `required` attr + no selected option with value
- If tag is `fieldset`/`form`: walk descendants, return true if any form element child is invalid
- All other elements: return false

For `PseudoClass::Valid`:
- Inverse of Invalid for form-related elements
- Non-form elements: return false

**2b. Add helper methods on DomElement:**

`is_form_element_invalid(&self) -> bool`:
- For `input`: has `required` AND (no `value` attr OR value is empty)
- For `textarea`: has `required` AND text content is empty
- For `select`: has `required` AND no selected option (check children for `selected` attr)

`has_invalid_descendant(&self) -> bool`:
- Walk all descendant elements recursively
- For each input/select/textarea descendant, check `is_form_element_invalid()`
- Return true if any is invalid

**Note on tree access:** The `DomElement` wrapper holds `&DomTree` (borrowed). To walk descendants, use `tree.get_node(id).children` and recurse. The tree is borrowed immutably so this is safe.

## Minimal Scope

Only implement what the test needs:
- Required + empty = invalid (for input, select, textarea)
- Fieldset/form invalid if any descendant form element is invalid
- Do NOT implement: email/URL/number validation, pattern matching, min/max/step, setCustomValidity

## Verification

Run only this test:
```bash
cargo test -p braille-engine --test wpt_dom -- "Element-closest.html"
```

Expected: 29/29 (up from 28/29).
