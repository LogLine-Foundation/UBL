use axum::{extract::{State}, http::HeaderMap, Json as AxumJson};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

use crate::ledger::Ledger;
use crate::types::*;
use crate::engine::{Kernel, ExecMeta, KeyMaterial};
use crate::interp;
use crate::trust_barrier;
use crate::error::UblError;
use uuid::Uuid;

fn require_auth(headers: &HeaderMap) -> Result<(), UblError> {
    if let Ok(expected) = std::env::var("UBL_API_KEY") {
        let got = headers.get("x-ubl-key").and_then(|h| h.to_str().ok()).unwrap_or("");
        if got != expected { return Err(UblError::Unauthorized); }
    }
    Ok(())
}

pub async fn execute(
    State(ledger): State<Arc<Ledger>>,
    headers: HeaderMap,
    AxumJson(req): AxumJson<ExecReq>,
) -> Result<AxumJson<Value>, UblError> {
    require_auth(&headers)?;

    let keys = KeyMaterial::from_env();
    let meta = ExecMeta { tx_id: Uuid::new_v4().to_string(), execution_time: chrono::Utc::now() };

    // Program
    let mut prog = ledger.get_program(&req.program)
        .ok_or_else(|| UblError::ProgramNotFound(req.program.clone()))?;
    prog.hash = Kernel::compute_program_hash(&prog);

    // Ledger snapshot root
    let ledger_root = ledger.snapshot_root();

    // Context binding
    // NOTE: we always include the full input object under `context.input`.
    // This lets program packs use either {field} or {input.field} templates.
    let mut ctx = serde_json::Map::new();
    ctx.insert("input".into(), req.inputs.clone());
    for c in &prog.context {
        match c.source {
            ContextSource::Input => {
                let p: Vec<String> = c.path.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                if let Some(v) = Kernel::resolve_path(&req.inputs, &p) { ctx.insert(c.name.clone(), v); }
            }
            ContextSource::Ledger => {
                // Interpolate using the already-bound context (ordered binding semantics).
                let ctx_val = Value::Object(ctx.clone());
                let resolved = interp::interpolate_str(&c.path, &ctx_val, None, &meta);
                let p: Vec<String> = resolved.split('.').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                if let Some(v) = Kernel::resolve_path(&ledger_root, &p) { ctx.insert(c.name.clone(), v); }
            }
            ContextSource::Computed => {
                if let Some(expr) = &c.expression {
                    let ctx_val = Value::Object(ctx.clone());
                    let v = Kernel::eval_expr(expr, &ctx_val, &meta);
                    ctx.insert(c.name.clone(), v);
                }
            }
        }
    }
    let context = Value::Object(ctx);

    // Chip (by hash or by `CHIP:<name>` reference)
    let mut chip = if prog.evaluate.starts_with("CHIP:") {
        let name = prog.evaluate.trim_start_matches("CHIP:");
        ledger.get_chip_by_name(name)
            .ok_or_else(|| UblError::ChipNotFound(prog.evaluate.clone()))?
    } else {
        ledger.get_chip(&prog.evaluate)
            .ok_or_else(|| UblError::ChipNotFound(prog.evaluate.clone()))?
    };
    chip.hash = Kernel::compute_chip_hash(&chip);

    // Proof
    let proof = Kernel::execute_chip_signed(&chip, &context, &meta, &keys);

    let allowed = proof.final_result == 1;
    let effects = if allowed { &prog.on_allow } else { &prog.on_deny };

    let input_hash = Kernel::jcs_hash(&req.inputs);

    let record = ledger.apply_transaction(
        &prog.hash, &input_hash, req.target_version, &proof, effects, &meta, &keys
    ).await?;

    info!("tx={} allowed={} version={}", meta.tx_id, allowed, record.resulting_version);

    Ok(AxumJson(json!({
        "tx_id": meta.tx_id,
        "allowed": allowed,
        "proof": proof,
        "effect_record": record
    })))
}

pub async fn register(
    State(ledger): State<Arc<Ledger>>,
    headers: HeaderMap,
    AxumJson(req): AxumJson<RegisterReq>,
) -> Result<AxumJson<Value>, UblError> {
    require_auth(&headers)?;
    match req {
        RegisterReq::Chip { data } => {
            let hash = ledger.register_chip(data)?;
            ledger.commit().await?;
            Ok(AxumJson(json!({ "hash": hash, "status": "registered" })))
        }
        RegisterReq::Program { data } => {
            let hash = ledger.register_program(data)?;
            ledger.commit().await?;
            Ok(AxumJson(json!({ "hash": hash, "status": "registered" })))
        }
    }
}

pub async fn list_chips(
    State(ledger): State<Arc<Ledger>>,
    headers: HeaderMap,
) -> Result<AxumJson<Value>, UblError> {
    require_auth(&headers)?;
    let xs = ledger.list_chips();
    Ok(AxumJson(json!({
        "chips": xs.iter().map(|(h,n,d)| json!({"hash":h,"name":n,"description":d})).collect::<Vec<_>>()
    })))
}

pub async fn list_programs(
    State(ledger): State<Arc<Ledger>>,
    headers: HeaderMap,
) -> Result<AxumJson<Value>, UblError> {
    require_auth(&headers)?;
    let xs = ledger.list_programs();
    Ok(AxumJson(json!({
        "programs": xs.iter().map(|(n,h)| json!({"name":n,"hash":h})).collect::<Vec<_>>()
    })))
}

pub async fn verify(
    State(ledger): State<Arc<Ledger>>,
    headers: HeaderMap,
    AxumJson(req): AxumJson<VerifyReq>,
) -> Result<AxumJson<Value>, UblError> {
    require_auth(&headers)?;
    let keys = KeyMaterial::from_env();

    let mut chip = ledger.get_chip(&req.proof.chip_hash)
        .ok_or_else(|| UblError::ChipNotFound(req.proof.chip_hash.clone()))?;
    chip.hash = Kernel::compute_chip_hash(&chip);

    let ok = Kernel::verify_proof(&req.proof, &chip, &keys);
    Ok(AxumJson(json!({"valid": ok})))
}

pub async fn barrier_process(
    headers: HeaderMap,
    AxumJson(req): AxumJson<BarrierReq>,
) -> Result<AxumJson<Value>, UblError> {
    require_auth(&headers)?;
    let vd = trust_barrier::process(&req)?;
    Ok(AxumJson(json!({"validated": vd})))
}

pub async fn health() -> AxumJson<Value> {
    AxumJson(json!({ "ok": true }))
}
