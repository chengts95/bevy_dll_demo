//! Param substitution at the serde Value level — the "special path".
//!
//! A pure `serde_json::Value` tree-walk, type-agnostic, run **before** rebuild.
//! It only fills in *values*; *references* (`$ref:`/`@ref:`) are left as
//! persistent names for a post-spawn resolution pass.

use std::collections::HashSet;

use crate::format::Snapshot;

/// Resolve `@global` references recursively in an arbitrary JSON value.
/// `$param`, `$ref:`, and `@ref:` symbols are intentionally left untouched.
pub fn try_resolve_globals(
    value: &serde_json::Value,
    globals: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    resolve_global_value(value, globals, &mut Vec::new())
}

fn resolve_global_value(
    value: &serde_json::Value,
    globals: &serde_json::Map<String, serde_json::Value>,
    stack: &mut Vec<String>,
) -> Result<serde_json::Value, String> {
    match value {
        serde_json::Value::String(symbol)
            if symbol.starts_with('@') && !symbol.starts_with("@ref:") =>
        {
            let name = &symbol[1..];
            let Some(global) = globals.get(name) else {
                return Err(format!("unknown global '@{name}'"));
            };
            if stack.iter().any(|item| item == name) {
                stack.push(name.to_string());
                return Err(format!("cyclic global reference: {}", stack.join(" -> ")));
            }
            stack.push(name.to_string());
            let resolved = resolve_global_value(global, globals, stack);
            stack.pop();
            resolved
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| resolve_global_value(item, globals, stack))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        serde_json::Value::Object(fields) => fields
            .iter()
            .map(|(name, item)| {
                resolve_global_value(item, globals, stack).map(|value| (name.clone(), value))
            })
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(serde_json::Value::Object),
        _ => Ok(value.clone()),
    }
}

/// Resolve symbol leaves in a flat template, in place:
///
/// - `"$x"`  → `params["x"]`   (local: this scope's params + ports)
/// - `"$x.y"` → `params["x"]["y"]` (object-path param lookup)
/// - `"@x"`  → `globals["x"]`  (global table, e.g. `@gnd`)
/// - `"$ref:label"` / `"@ref:label"` → **left untouched** (entity references,
///   resolved to `Entity` handles in a post-spawn pass, not substituted here).
pub fn substitute(
    snap: &mut Snapshot,
    params: &serde_json::Map<String, serde_json::Value>,
    globals: &serde_json::Map<String, serde_json::Value>,
) {
    try_substitute(snap, params, globals).unwrap_or_else(|err| panic!("{err}"));
}

/// Fallible variant of [`substitute`]. Missing `$param` / `@global` references
/// are reported with record/component context instead of being left unresolved.
pub fn try_substitute(
    snap: &mut Snapshot,
    params: &serde_json::Map<String, serde_json::Value>,
    globals: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    for rec in snap {
        for (component, v) in rec.components.iter_mut() {
            substitute_value(
                v,
                params,
                globals,
                &format!("record {} component '{component}'", rec.id),
            )?;
        }
    }
    Ok(())
}

fn substitute_value(
    value: &mut serde_json::Value,
    params: &serde_json::Map<String, serde_json::Value>,
    globals: &serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<(), String> {
    match value {
        serde_json::Value::String(symbol) => {
            if let Some(replacement) = resolve_symbol(symbol, params, globals, path)? {
                *value = replacement;
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                substitute_value(item, params, globals, &array_path(path, index))?;
            }
        }
        serde_json::Value::Object(fields) => {
            for (field, item) in fields.iter_mut() {
                substitute_value(item, params, globals, &field_path(path, field))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn resolve_symbol(
    symbol: &str,
    params: &serde_json::Map<String, serde_json::Value>,
    globals: &serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Result<Option<serde_json::Value>, String> {
    if symbol.starts_with("$ref:") || symbol.starts_with("@ref:") || symbol.starts_with("$id:") {
        return Ok(None);
    }
    if let Some(name) = symbol.strip_prefix('@') {
        return globals
            .get(name)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{path}: unknown global '@{name}'"));
    }
    if let Some(name) = symbol.strip_prefix('$') {
        return lookup_param_path(params, name)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{path}: unknown param '${name}'"));
    }
    Ok(None)
}

pub(crate) fn is_param_symbol(symbol: &str) -> bool {
    symbol.starts_with('$') && !symbol.starts_with("$ref:") && !symbol.starts_with("$id:")
}

pub(crate) fn param_name(symbol: &str) -> Option<&str> {
    symbol
        .strip_prefix('$')
        .filter(|name| !name.starts_with("ref:") && !name.starts_with("id:") && !name.is_empty())
}

pub(crate) fn lookup_param_path<'a>(
    params: &'a serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Option<&'a serde_json::Value> {
    if let Some(value) = params.get(path) {
        return Some(value);
    }

    let mut segments = path.split('.');
    let first = segments.next()?;
    if first.is_empty() {
        return None;
    }
    let mut value = params.get(first)?;
    for segment in segments {
        if segment.is_empty() {
            return None;
        }
        value = value.as_object()?.get(segment)?;
    }
    Some(value)
}

pub(crate) fn lookup_param_path_mut<'a>(
    params: &'a mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Option<&'a mut serde_json::Value> {
    if params.contains_key(path) {
        return params.get_mut(path);
    }

    let mut segments = path.split('.');
    let first = segments.next()?;
    if first.is_empty() {
        return None;
    }
    let mut value = params.get_mut(first)?;
    for segment in segments {
        if segment.is_empty() {
            return None;
        }
        value = value.as_object_mut()?.get_mut(segment)?;
    }
    Some(value)
}

fn array_path(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

fn field_path(parent: &str, field: &str) -> String {
    format!("{parent}.{field}")
}

/// Prefix a nested instance's **internal** net names with `prefix`, so two
/// instances of the same prefab don't collide (and stay individually probeable,
/// SPICE-style `X1.mid`). Touches only registered net components; leaves
/// `$pin` (ports) and `@x` (globals) untouched. Runs **before** `substitute`.
/// An empty `prefix` is a no-op (top-level nets stay as authored).
pub fn qualify(snap: &mut Snapshot, net_components: &HashSet<String>, prefix: &str) {
    if prefix.is_empty() {
        return;
    }
    fn leaf(v: &mut serde_json::Value, prefix: &str) {
        match v {
            serde_json::Value::String(s) => {
                if !s.starts_with('$') && !s.starts_with('@') {
                    *s = format!("{prefix}{s}");
                }
            }
            serde_json::Value::Array(items) => {
                items.iter_mut().for_each(|item| leaf(item, prefix));
            }
            serde_json::Value::Object(fields) => {
                fields.values_mut().for_each(|item| leaf(item, prefix));
            }
            _ => {}
        }
    }
    for rec in snap {
        for (cname, val) in rec.components.iter_mut() {
            if net_components.contains(cname) {
                leaf(val, prefix);
            }
        }
    }
}
