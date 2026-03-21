use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, FunctionObjectBuilder, ObjectInitializer},
    property::PropertyDescriptor,
    Context, JsObject, JsValue,
};

use crate::js::prop_desc;

// ---------------------------------------------------------------------------
// JsUrl — native data wrapping url::Url
// ---------------------------------------------------------------------------

#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsUrl {
    #[unsafe_ignore_trace]
    inner: Rc<RefCell<url::Url>>,
}

// ---------------------------------------------------------------------------
// JsURLSearchParams — native data, optionally backed by a URL
// ---------------------------------------------------------------------------

#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsURLSearchParams {
    /// If Some, mutations sync back to the parent URL via set_query().
    #[unsafe_ignore_trace]
    parent_url: Option<Rc<RefCell<url::Url>>>,
    /// Standalone storage (used when parent_url is None).
    #[unsafe_ignore_trace]
    entries: Rc<RefCell<Vec<(String, String)>>>,
}

impl JsURLSearchParams {
    fn pairs(&self) -> Vec<(String, String)> {
        if let Some(ref url_rc) = self.parent_url {
            let url = url_rc.borrow();
            url.query_pairs().into_owned().collect()
        } else {
            self.entries.borrow().clone()
        }
    }

    fn set_pairs(&self, pairs: &[(String, String)]) {
        if let Some(ref url_rc) = self.parent_url {
            let mut url = url_rc.borrow_mut();
            if pairs.is_empty() {
                url.set_query(None);
            } else {
                let qs: String = url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(pairs)
                    .finish();
                url.set_query(Some(&qs));
            }
        } else {
            *self.entries.borrow_mut() = pairs.to_vec();
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: downcast to JsUrl / JsURLSearchParams
// ---------------------------------------------------------------------------

fn get_url(this: &JsValue, method: &str) -> Result<Rc<RefCell<url::Url>>, boa_engine::JsError> {
    let obj = this
        .as_object()
        .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("URL.{method} called on non-object")))?;
    let data = obj
        .downcast_ref::<JsUrl>()
        .ok_or_else(|| boa_engine::JsNativeError::typ().with_message(format!("URL.{method} called on non-URL")))?;
    Ok(Rc::clone(&data.inner))
}

fn with_usp<F, R>(this: &JsValue, method: &str, f: F) -> Result<R, boa_engine::JsError>
where
    F: FnOnce(&JsURLSearchParams) -> R,
{
    let obj = this.as_object().ok_or_else(|| {
        boa_engine::JsNativeError::typ().with_message(format!("URLSearchParams.{method} called on non-object"))
    })?;
    let data = obj.downcast_ref::<JsURLSearchParams>().ok_or_else(|| {
        boa_engine::JsNativeError::typ().with_message(format!("URLSearchParams.{method} called on non-URLSearchParams"))
    })?;
    Ok(f(&data))
}

// ---------------------------------------------------------------------------
// URL constructor + prototype
// ---------------------------------------------------------------------------

pub(crate) fn register_url_globals(ctx: &mut Context) {
    // -----------------------------------------------------------------------
    // URL prototype
    // -----------------------------------------------------------------------
    let proto = ObjectInitializer::new(ctx).build();
    let realm = ctx.realm().clone();

    // --- Getters/setters ---

    // href getter
    let href_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "href")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.as_str().to_string())))
    });
    // href setter
    let href_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "href")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let parsed = url::Url::parse(&val)
            .map_err(|_| boa_engine::JsNativeError::typ().with_message(format!("Invalid URL: {val}")))?;
        *url_rc.borrow_mut() = parsed;
        Ok(JsValue::undefined())
    });

    // origin (read-only)
    let origin_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "origin")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.origin().unicode_serialization())))
    });

    // protocol
    let protocol_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "protocol")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(format!("{}:", url.scheme()))))
    });
    let protocol_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "protocol")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let scheme = val.trim_end_matches(':');
        let _ = url_rc.borrow_mut().set_scheme(scheme);
        Ok(JsValue::undefined())
    });

    // host
    let host_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "host")?;
        let url = url_rc.borrow();
        let host = match (url.host_str(), url.port()) {
            (Some(h), Some(p)) => format!("{h}:{p}"),
            (Some(h), None) => h.to_string(),
            _ => String::new(),
        };
        Ok(JsValue::from(js_string!(host)))
    });
    let host_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "host")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let _ = url_rc.borrow_mut().set_host(Some(&val));
        Ok(JsValue::undefined())
    });

    // hostname
    let hostname_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "hostname")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.host_str().unwrap_or("").to_string())))
    });
    let hostname_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "hostname")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let _ = url_rc.borrow_mut().set_host(Some(&val));
        Ok(JsValue::undefined())
    });

    // port
    let port_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "port")?;
        let url = url_rc.borrow();
        match url.port() {
            Some(p) => Ok(JsValue::from(js_string!(p.to_string()))),
            None => Ok(JsValue::from(js_string!(""))),
        }
    });
    let port_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "port")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        if val.is_empty() {
            let _ = url_rc.borrow_mut().set_port(None);
        } else if let Ok(p) = val.parse::<u16>() {
            let _ = url_rc.borrow_mut().set_port(Some(p));
        }
        Ok(JsValue::undefined())
    });

    // pathname
    let pathname_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "pathname")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.path().to_string())))
    });
    let pathname_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "pathname")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        url_rc.borrow_mut().set_path(&val);
        Ok(JsValue::undefined())
    });

    // search
    let search_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "search")?;
        let url = url_rc.borrow();
        match url.query() {
            Some(q) => Ok(JsValue::from(js_string!(format!("?{q}")))),
            None => Ok(JsValue::from(js_string!(""))),
        }
    });
    let search_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "search")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let q = val.strip_prefix('?').unwrap_or(&val);
        if q.is_empty() {
            url_rc.borrow_mut().set_query(None);
        } else {
            url_rc.borrow_mut().set_query(Some(q));
        }
        Ok(JsValue::undefined())
    });

    // hash
    let hash_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "hash")?;
        let url = url_rc.borrow();
        match url.fragment() {
            Some(f) => Ok(JsValue::from(js_string!(format!("#{f}")))),
            None => Ok(JsValue::from(js_string!(""))),
        }
    });
    let hash_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "hash")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let f = val.strip_prefix('#').unwrap_or(&val);
        if f.is_empty() {
            url_rc.borrow_mut().set_fragment(None);
        } else {
            url_rc.borrow_mut().set_fragment(Some(f));
        }
        Ok(JsValue::undefined())
    });

    // username
    let username_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "username")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.username().to_string())))
    });
    let username_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "username")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let _ = url_rc.borrow_mut().set_username(&val);
        Ok(JsValue::undefined())
    });

    // password
    let password_get = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "password")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.password().unwrap_or("").to_string())))
    });
    let password_set = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let url_rc = get_url(this, "password")?;
        let val = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let _ = url_rc.borrow_mut().set_password(Some(&val));
        Ok(JsValue::undefined())
    });

    // toString → href
    let to_string_fn = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "toString")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.as_str().to_string())))
    });

    // toJSON → href
    let to_json_fn = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let url_rc = get_url(this, "toJSON")?;
        let url = url_rc.borrow();
        Ok(JsValue::from(js_string!(url.as_str().to_string())))
    });

    // Define accessors on prototype
    macro_rules! define_accessor {
        ($proto:expr, $name:expr, $getter:expr, $setter:expr, $ctx:expr) => {
            $proto
                .define_property_or_throw(
                    js_string!($name),
                    PropertyDescriptor::builder()
                        .get($getter.to_js_function(&realm))
                        .set($setter.to_js_function(&realm))
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    $ctx,
                )
                .expect(concat!("define URL.prototype.", $name));
        };
    }
    macro_rules! define_readonly_accessor {
        ($proto:expr, $name:expr, $getter:expr, $ctx:expr) => {
            $proto
                .define_property_or_throw(
                    js_string!($name),
                    prop_desc::readonly_accessor($getter.to_js_function(&realm)),
                    $ctx,
                )
                .expect(concat!("define URL.prototype.", $name));
        };
    }

    define_accessor!(proto, "href", href_get, href_set, ctx);
    define_readonly_accessor!(proto, "origin", origin_get, ctx);
    define_accessor!(proto, "protocol", protocol_get, protocol_set, ctx);
    define_accessor!(proto, "host", host_get, host_set, ctx);
    define_accessor!(proto, "hostname", hostname_get, hostname_set, ctx);
    define_accessor!(proto, "port", port_get, port_set, ctx);
    define_accessor!(proto, "pathname", pathname_get, pathname_set, ctx);
    define_accessor!(proto, "search", search_get, search_set, ctx);
    define_accessor!(proto, "hash", hash_get, hash_set, ctx);
    define_accessor!(proto, "username", username_get, username_set, ctx);
    define_accessor!(proto, "password", password_get, password_set, ctx);

    proto
        .define_property_or_throw(js_string!("toString"), prop_desc::data_prop(to_string_fn.to_js_function(&realm)), ctx)
        .expect("define URL.prototype.toString");
    proto
        .define_property_or_throw(js_string!("toJSON"), prop_desc::data_prop(to_json_fn.to_js_function(&realm)), ctx)
        .expect("define URL.prototype.toJSON");

    // searchParams getter — lazily caches a URLSearchParams on the instance
    let usp_proto_for_search_params = register_url_search_params(ctx);
    let sp_proto_clone = usp_proto_for_search_params.clone();
    let search_params_get = unsafe {
        NativeFunction::from_closure(move |this, _args, ctx| {
            let obj = this.as_object().ok_or_else(|| {
                boa_engine::JsNativeError::typ().with_message("URL.searchParams called on non-object")
            })?;
            // Check for cached __searchParams
            let cached = obj.get(js_string!("__searchParams"), ctx)?;
            if !cached.is_undefined() && !cached.is_null() {
                return Ok(cached);
            }
            // Create URLSearchParams backed by this URL
            // Extract Rc before dropping the downcast borrow (to avoid BorrowMutError on obj.set)
            let parent_rc = {
                let url_data = obj.downcast_ref::<JsUrl>().ok_or_else(|| {
                    boa_engine::JsNativeError::typ().with_message("URL.searchParams called on non-URL")
                })?;
                Rc::clone(&url_data.inner)
            };
            let usp = JsURLSearchParams {
                parent_url: Some(parent_rc),
                entries: Rc::new(RefCell::new(Vec::new())),
            };
            let usp_obj = ObjectInitializer::with_native_data(usp, ctx).build();
            usp_obj.set_prototype(Some(sp_proto_clone.clone()));
            obj.set(js_string!("__searchParams"), JsValue::from(usp_obj.clone()), false, ctx)?;
            Ok(JsValue::from(usp_obj))
        })
    };
    proto
        .define_property_or_throw(
            js_string!("searchParams"),
            prop_desc::readonly_accessor(search_params_get.to_js_function(&realm)),
            ctx,
        )
        .expect("define URL.prototype.searchParams");

    // Symbol.toStringTag
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            PropertyDescriptor::builder()
                .value(js_string!("URL"))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("define URL.prototype[Symbol.toStringTag]");

    // -----------------------------------------------------------------------
    // URL constructor
    // -----------------------------------------------------------------------
    let proto_for_ctor = proto.clone();
    let url_ctor_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let input = args
                .first()
                .map(|v| v.to_string(ctx))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let parsed = if let Some(base_val) = args.get(1) {
                if !base_val.is_undefined() {
                    let base_str = base_val.to_string(ctx)?.to_std_string_escaped();
                    let base = url::Url::parse(&base_str)
                        .map_err(|_| boa_engine::JsNativeError::typ().with_message(format!("Invalid base URL: {base_str}")))?;
                    base.join(&input)
                        .map_err(|_| boa_engine::JsNativeError::typ().with_message(format!("Invalid URL: {input}")))?
                } else {
                    url::Url::parse(&input)
                        .map_err(|_| boa_engine::JsNativeError::typ().with_message(format!("Invalid URL: {input}")))?
                }
            } else {
                url::Url::parse(&input)
                    .map_err(|_| boa_engine::JsNativeError::typ().with_message(format!("Invalid URL: {input}")))?
            };

            let data = JsUrl {
                inner: Rc::new(RefCell::new(parsed)),
            };
            let obj = ObjectInitializer::with_native_data(data, ctx).build();
            obj.set_prototype(Some(proto_for_ctor.clone()));
            Ok(JsValue::from(obj))
        })
    };

    let ctor: JsObject = FunctionObjectBuilder::new(ctx.realm(), url_ctor_fn)
        .name(js_string!("URL"))
        .length(1)
        .constructor(true)
        .build()
        .into();

    ctor.define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("set URL.prototype");
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor.clone()), ctx)
        .expect("set URL.prototype.constructor");

    ctx.global_object()
        .set(js_string!("URL"), JsValue::from(ctor), false, ctx)
        .expect("set URL global");
}

// ---------------------------------------------------------------------------
// URLSearchParams constructor + prototype
// ---------------------------------------------------------------------------

fn register_url_search_params(ctx: &mut Context) -> JsObject {
    let proto = ObjectInitializer::new(ctx).build();
    let realm = ctx.realm().clone();

    // --- get(name) ---
    let get_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let name = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        with_usp(this, "get", |usp| {
            let pairs = usp.pairs();
            for (k, v) in &pairs {
                if k == &name {
                    return JsValue::from(js_string!(v.clone()));
                }
            }
            JsValue::null()
        })
    });

    // --- getAll(name) ---
    let get_all_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let name = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        with_usp(this, "getAll", |usp| {
            let pairs = usp.pairs();
            let arr = JsArray::new(ctx);
            for (k, v) in &pairs {
                if k == &name {
                    arr.push(JsValue::from(js_string!(v.clone())), ctx).unwrap();
                }
            }
            JsValue::from(arr)
        })
    });

    // --- has(name) ---
    let has_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let name = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        with_usp(this, "has", |usp| {
            let pairs = usp.pairs();
            JsValue::from(pairs.iter().any(|(k, _)| k == &name))
        })
    });

    // --- set(name, value) ---
    let set_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let name = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let value = args.get(1).map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        with_usp(this, "set", |usp| {
            let mut pairs = usp.pairs();
            // Remove all existing, then add one
            pairs.retain(|(k, _)| k != &name);
            pairs.push((name, value));
            usp.set_pairs(&pairs);
            JsValue::undefined()
        })
    });

    // --- append(name, value) ---
    let append_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let name = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        let value = args.get(1).map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        with_usp(this, "append", |usp| {
            let mut pairs = usp.pairs();
            pairs.push((name, value));
            usp.set_pairs(&pairs);
            JsValue::undefined()
        })
    });

    // --- delete(name) ---
    let delete_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let name = args.first().map(|v| v.to_string(ctx)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
        with_usp(this, "delete", |usp| {
            let mut pairs = usp.pairs();
            pairs.retain(|(k, _)| k != &name);
            usp.set_pairs(&pairs);
            JsValue::undefined()
        })
    });

    // --- sort() ---
    let sort_fn = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        with_usp(this, "sort", |usp| {
            let mut pairs = usp.pairs();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            usp.set_pairs(&pairs);
            JsValue::undefined()
        })
    });

    // --- toString() ---
    let to_string_fn = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        with_usp(this, "toString", |usp| {
            let pairs = usp.pairs();
            let qs = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(&pairs)
                .finish();
            JsValue::from(js_string!(qs))
        })
    });

    // --- entries() ---
    let entries_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        with_usp(this, "entries", |usp| {
            let pairs = usp.pairs();
            let arr = JsArray::new(ctx);
            for (k, v) in &pairs {
                let pair = JsArray::new(ctx);
                pair.push(JsValue::from(js_string!(k.clone())), ctx).unwrap();
                pair.push(JsValue::from(js_string!(v.clone())), ctx).unwrap();
                arr.push(JsValue::from(pair), ctx).unwrap();
            }
            let iterator_fn = arr.get(boa_engine::JsSymbol::iterator(), ctx).unwrap();
            if let Some(callable) = iterator_fn.as_callable() {
                return callable.call(&JsValue::from(arr), &[], ctx).unwrap();
            }
            JsValue::from(arr)
        })
    });

    // --- keys() ---
    let keys_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        with_usp(this, "keys", |usp| {
            let pairs = usp.pairs();
            let arr = JsArray::new(ctx);
            for (k, _) in &pairs {
                arr.push(JsValue::from(js_string!(k.clone())), ctx).unwrap();
            }
            let iterator_fn = arr.get(boa_engine::JsSymbol::iterator(), ctx).unwrap();
            if let Some(callable) = iterator_fn.as_callable() {
                return callable.call(&JsValue::from(arr), &[], ctx).unwrap();
            }
            JsValue::from(arr)
        })
    });

    // --- values() ---
    let values_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        with_usp(this, "values", |usp| {
            let pairs = usp.pairs();
            let arr = JsArray::new(ctx);
            for (_, v) in &pairs {
                arr.push(JsValue::from(js_string!(v.clone())), ctx).unwrap();
            }
            let iterator_fn = arr.get(boa_engine::JsSymbol::iterator(), ctx).unwrap();
            if let Some(callable) = iterator_fn.as_callable() {
                return callable.call(&JsValue::from(arr), &[], ctx).unwrap();
            }
            JsValue::from(arr)
        })
    });

    // --- forEach(callback) ---
    let for_each_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let callback = args.first().cloned().unwrap_or(JsValue::undefined());
        let callable = callback.as_callable().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("URLSearchParams.forEach callback is not callable")
        })?;
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("URLSearchParams.forEach called on non-object")
        })?;
        let usp = obj.downcast_ref::<JsURLSearchParams>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("URLSearchParams.forEach called on non-URLSearchParams")
        })?;
        let pairs = usp.pairs();
        drop(usp);
        for (k, v) in pairs {
            callable.call(
                &JsValue::undefined(),
                &[
                    JsValue::from(js_string!(v)),
                    JsValue::from(js_string!(k)),
                    this.clone(),
                ],
                ctx,
            )?;
        }
        Ok(JsValue::undefined())
    });

    // Define all methods on prototype
    for (name, func) in [
        ("get", &get_fn),
        ("getAll", &get_all_fn),
        ("has", &has_fn),
        ("set", &set_fn),
        ("append", &append_fn),
        ("delete", &delete_fn),
        ("sort", &sort_fn),
        ("toString", &to_string_fn),
        ("entries", &entries_fn),
        ("keys", &keys_fn),
        ("values", &values_fn),
        ("forEach", &for_each_fn),
    ] {
        proto
            .define_property_or_throw(js_string!(name), prop_desc::data_prop(func.clone().to_js_function(&realm)), ctx)
            .unwrap_or_else(|_| panic!("define URLSearchParams.prototype.{name}"));
    }

    // Symbol.iterator → entries
    let iter_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("URLSearchParams[Symbol.iterator] called on non-object")
        })?;
        let entries_method = obj.get(js_string!("entries"), ctx)?;
        if let Some(callable) = entries_method.as_callable() {
            return callable.call(this, args, ctx);
        }
        Ok(JsValue::undefined())
    });
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::iterator(),
            prop_desc::data_prop(iter_fn.to_js_function(&realm)),
            ctx,
        )
        .expect("define URLSearchParams.prototype[Symbol.iterator]");

    // Symbol.toStringTag
    proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            PropertyDescriptor::builder()
                .value(js_string!("URLSearchParams"))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("define URLSearchParams.prototype[Symbol.toStringTag]");

    // --- Constructor ---
    let proto_for_ctor = proto.clone();
    let usp_ctor_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let entries = if let Some(init) = args.first() {
                if init.is_undefined() || init.is_null() {
                    Vec::new()
                } else {
                    let s = init.to_string(ctx)?.to_std_string_escaped();
                    let q = s.strip_prefix('?').unwrap_or(&s);
                    url::form_urlencoded::parse(q.as_bytes())
                        .into_owned()
                        .collect()
                }
            } else {
                Vec::new()
            };

            let data = JsURLSearchParams {
                parent_url: None,
                entries: Rc::new(RefCell::new(entries)),
            };
            let obj = ObjectInitializer::with_native_data(data, ctx).build();
            obj.set_prototype(Some(proto_for_ctor.clone()));
            Ok(JsValue::from(obj))
        })
    };

    let ctor: JsObject = FunctionObjectBuilder::new(ctx.realm(), usp_ctor_fn)
        .name(js_string!("URLSearchParams"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor.define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("set URLSearchParams.prototype");
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor.clone()), ctx)
        .expect("set URLSearchParams.prototype.constructor");

    ctx.global_object()
        .set(js_string!("URLSearchParams"), JsValue::from(ctor), false, ctx)
        .expect("set URLSearchParams global");

    proto
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::dom::DomTree;
    use crate::js::JsRuntime;

    fn make_runtime() -> JsRuntime {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
        }
        JsRuntime::new(tree)
    }

    #[test]
    fn url_constructor_absolute() {
        let mut rt = make_runtime();
        let r = rt.eval("var u = new URL('https://example.com/path?q=1#h'); u.href").unwrap();
        assert_eq!(r.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "https://example.com/path?q=1#h");
    }

    #[test]
    fn url_constructor_relative_with_base() {
        let mut rt = make_runtime();
        let r = rt.eval("new URL('/foo', 'https://example.com').href").unwrap();
        assert_eq!(r.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "https://example.com/foo");
    }

    #[test]
    fn url_constructor_throws_on_invalid() {
        let mut rt = make_runtime();
        let r = rt.eval("try { new URL('not a url'); false } catch(e) { e instanceof TypeError }").unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_getters() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var u = new URL('https://user:pass@example.com:8080/path?q=1#hash');
            u.protocol === 'https:' &&
            u.hostname === 'example.com' &&
            u.port === '8080' &&
            u.pathname === '/path' &&
            u.search === '?q=1' &&
            u.hash === '#hash' &&
            u.username === 'user' &&
            u.password === 'pass'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_origin() {
        let mut rt = make_runtime();
        let r = rt.eval("new URL('https://example.com:443/path').origin").unwrap();
        assert_eq!(r.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "https://example.com");
    }

    #[test]
    fn url_setters() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var u = new URL('https://example.com/old');
            u.pathname = '/new';
            u.search = '?x=1';
            u.hash = '#top';
            u.href
        "#).unwrap();
        assert_eq!(r.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "https://example.com/new?x=1#top");
    }

    #[test]
    fn url_to_string() {
        let mut rt = make_runtime();
        let r = rt.eval("new URL('https://example.com').toString()").unwrap();
        assert_eq!(r.to_string(&mut rt.context).unwrap().to_std_string_escaped(), "https://example.com/");
    }

    #[test]
    fn url_search_params_from_url() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var u = new URL('https://example.com?a=1&b=2');
            var sp = u.searchParams;
            sp.get('a') === '1' && sp.get('b') === '2'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_sync_to_url() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var u = new URL('https://example.com?a=1');
            u.searchParams.set('a', '2');
            u.searchParams.append('c', '3');
            u.search === '?a=2&c=3'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_cached() {
        let mut rt = make_runtime();
        let r = rt.eval("var u = new URL('https://example.com'); u.searchParams === u.searchParams").unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_standalone() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var sp = new URLSearchParams('a=1&b=2');
            sp.get('a') === '1' && sp.get('b') === '2' && sp.toString() === 'a=1&b=2'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_has_delete() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var sp = new URLSearchParams('x=1');
            var had = sp.has('x');
            sp.delete('x');
            had && !sp.has('x')
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_sort() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var sp = new URLSearchParams('c=3&a=1&b=2');
            sp.sort();
            sp.toString() === 'a=1&b=2&c=3'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_iteration() {
        let mut rt = make_runtime();
        let r = rt.eval(r#"
            var sp = new URLSearchParams('x=1&y=2');
            var out = [];
            for (var pair of sp) { out.push(pair[0] + '=' + pair[1]); }
            out.join('&') === 'x=1&y=2'
        "#).unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_instanceof() {
        let mut rt = make_runtime();
        let r = rt.eval("new URL('https://example.com') instanceof URL").unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }

    #[test]
    fn url_search_params_instanceof() {
        let mut rt = make_runtime();
        let r = rt.eval("new URLSearchParams() instanceof URLSearchParams").unwrap();
        assert_eq!(r.as_boolean(), Some(true));
    }
}
