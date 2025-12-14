use serde_json::{Map, Value};

use crate::engine::{ExecMeta, Kernel};
use crate::types::Proof;

/// Deterministic placeholder interpolation for template strings.
///
/// Supported tokens (both `{x}` and `{{x}}` forms):
/// - `{now}` / `{tx_id}`
/// - `{proof.failed_gates}` (only if `proof` is provided)
/// - Any context path: `{sender.balance}`
///
/// Convenience fallbacks:
/// - `{amount}` will resolve to `ctx.amount` OR `ctx.input.amount` if present
/// - `{input.amount}` always works if `ctx.input` exists
pub fn interpolate_str(template: &str, ctx: &Value, proof: Option<&Proof>, meta: &ExecMeta) -> String {
    let mut out = template.to_string();

    // Special tokens
    let now = Kernel::now_rfc3339(meta);
    out = out.replace("{now}", &now);
    out = out.replace("{{now}}", &now);

    out = out.replace("{tx_id}", &meta.tx_id);
    out = out.replace("{{tx_id}}", &meta.tx_id);

    if let Some(p) = proof {
        let fg = serde_json::to_string(&p.failed_gates).unwrap_or_else(|_| "[]".into());
        out = out.replace("{proof.failed_gates}", &fg);
        out = out.replace("{{proof.failed_gates}}", &fg);
    }

    // Token resolver: {path} and {{path}}
    for (open, close) in [("{", "}"), ("{{", "}}")] {
        loop {
            let start = match out.find(open) {
                Some(s) => s,
                None => break,
            };
            let rest = &out[start + open.len()..];
            let end_rel = match rest.find(close) {
                Some(e) => e,
                None => break,
            };
            let end = start + open.len() + end_rel;

            let token = out[start + open.len()..end].trim().to_string();

            // NOTE: by this point we already replaced the most common token forms,
            // but we still handle them here to avoid surprising outputs.
            let replacement = match token.as_str() {
                "now" => Some(Value::String(now.clone())),
                "tx_id" => Some(Value::String(meta.tx_id.clone())),
                "proof.failed_gates" => proof.map(|p| {
                    Value::String(serde_json::to_string(&p.failed_gates).unwrap_or_else(|_| "[]".into()))
                }),
                _ => resolve_token(ctx, &token),
            };

            if let Some(v) = replacement {
                let s = if v.is_string() {
                    v.as_str().unwrap_or("").to_string()
                } else {
                    v.to_string()
                };
                out.replace_range(start..(end + close.len()), &s);
            } else {
                // Strip braces but keep token visible.
                out.replace_range(start..(end + close.len()), &token);
            }
        }
    }

    out
}

fn resolve_token(ctx: &Value, token: &str) -> Option<Value> {
    let parts: Vec<String> = token
        .split('.')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if parts.is_empty() {
        return None;
    }

    // 1) direct resolve
    if let Some(v) = Kernel::resolve_path(ctx, &parts) {
        return Some(v);
    }

    // 2) fallback: treat `{x}` as `{input.x}` (very common for program packs)
    if parts.len() == 1 {
        let p2 = vec!["input".to_string(), parts[0].clone()];
        if let Some(v) = Kernel::resolve_path(ctx, &p2) {
            return Some(v);
        }
    } else if parts[0] != "input" {
        let mut p2 = vec!["input".to_string()];
        p2.extend(parts.clone());
        if let Some(v) = Kernel::resolve_path(ctx, &p2) {
            return Some(v);
        }
    }

    None
}

/// Recursively interpolates template strings within a JSON value.
/// - strings are interpolated via [`interpolate_str`]
/// - arrays/objects are traversed
pub fn interpolate_value(v: &Value, ctx: &Value, proof: Option<&Proof>, meta: &ExecMeta) -> Value {
    match v {
        Value::String(s) => Value::String(interpolate_str(s, ctx, proof, meta)),
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|x| interpolate_value(x, ctx, proof, meta))
                .collect(),
        ),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, vv) in map {
                out.insert(k.clone(), interpolate_value(vv, ctx, proof, meta));
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}
