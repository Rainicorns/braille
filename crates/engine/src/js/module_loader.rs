use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rquickjs::loader::{Loader, Resolver};
use rquickjs::{Ctx, Error, Module, Result};

/// Shared module registry that holds:
/// - `modules`: specifier -> source code for pre-registered modules
/// - `import_map`: bare specifier -> resolved specifier (from import maps)
#[derive(Debug, Default)]
pub struct ModuleRegistry {
    /// Module source code keyed by specifier
    pub modules: HashMap<String, String>,
    /// Import map: bare specifier -> resolved specifier
    pub import_map: HashMap<String, String>,
}

pub type SharedModuleRegistry = Rc<RefCell<ModuleRegistry>>;

pub fn new_registry() -> SharedModuleRegistry {
    Rc::new(RefCell::new(ModuleRegistry::default()))
}

/// Custom resolver that:
/// 1. Applies import map remapping (bare specifier -> URL)
/// 2. Resolves relative paths against the base module
/// 3. Checks that the resolved name exists in the registry
pub struct BrailleResolver {
    pub registry: SharedModuleRegistry,
}

impl Resolver for BrailleResolver {
    fn resolve<'js>(&mut self, _ctx: &Ctx<'js>, base: &str, name: &str) -> Result<String> {
        let reg = self.registry.borrow();

        // First check import map
        if let Some(mapped) = reg.import_map.get(name) {
            // The mapped value might itself be a module name in the registry
            // or a URL. Check if the mapped value is registered directly.
            if reg.modules.contains_key(mapped) {
                return Ok(mapped.clone());
            }
            // The mapped value might be the same as a bare specifier that
            // was registered directly (e.g., import map maps "my-lib" -> "./my-lib.js"
            // and "./my-lib.js" is registered). Return mapped.
            return Ok(mapped.clone());
        }

        // Resolve relative paths
        let resolved = if name.starts_with('.') {
            // Resolve relative to base directory
            let mut split = base.rsplitn(2, '/');
            let path = match (split.next(), split.next()) {
                (_, Some(path)) => path,
                _ => "",
            };
            if path.is_empty() {
                name.trim_start_matches("./").to_string()
            } else {
                format!("{path}/{}", name.trim_start_matches("./"))
            }
        } else {
            name.to_string()
        };

        // Check if the resolved module exists in registry
        if reg.modules.contains_key(&resolved) {
            return Ok(resolved);
        }

        // Also try the name as-is (absolute specifiers)
        if reg.modules.contains_key(name) {
            return Ok(name.to_string());
        }

        Err(Error::new_resolving(base, name))
    }
}

/// Custom loader that serves module source from the shared registry.
pub struct BrailleLoader {
    pub registry: SharedModuleRegistry,
}

impl Loader for BrailleLoader {
    fn load<'js>(&mut self, ctx: &Ctx<'js>, name: &str) -> Result<Module<'js, rquickjs::module::Declared>> {
        let source = {
            let reg = self.registry.borrow();
            reg.modules.get(name).cloned()
        };

        match source {
            Some(source) => Module::declare(ctx.clone(), name, source),
            None => Err(Error::new_loading(name)),
        }
    }
}
