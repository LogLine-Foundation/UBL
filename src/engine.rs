use crate::types::*;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use chrono::{DateTime, Utc};
use serde_jcs::to_string as jcs_to_string;

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

pub struct Kernel;

#[derive(Clone)]
pub struct ExecMeta {
    pub tx_id: String,
    pub execution_time: DateTime<Utc>,
}

#[derive(Clone)]
pub struct KeyMaterial {
    pub signing: Option<SigningKey>,
    pub verifying: Option<VerifyingKey>,
}

impl KeyMaterial {
    pub fn from_env() -> Self {
        let priv_b64 = std::env::var("UBL_ED25519_PRIVATE_KEY_B64").ok();
        let pub_b64  = std::env::var("UBL_ED25519_PUBLIC_KEY_B64").ok();

        let signing: Option<SigningKey> = priv_b64
            .and_then(|s| B64.decode(s).ok())
            .and_then(|b| {
                let arr: [u8; 32] = b.as_slice().try_into().ok()?;
                Some(SigningKey::from_bytes(&arr))
            });

        let verifying: Option<VerifyingKey> = pub_b64
            .and_then(|s| B64.decode(s).ok())
            .and_then(|b| {
                let arr: [u8; 32] = b.as_slice().try_into().ok()?;
                VerifyingKey::from_bytes(&arr).ok()
            })
            .or_else(|| signing.as_ref().map(|sk| sk.verifying_key()));

        Self { signing, verifying }
    }

    pub fn sign_b64(&self, msg: &[u8]) -> Option<String> {
        self.signing.as_ref().map(|sk| {
            let sig: Signature = sk.sign(msg);
            B64.encode(sig.to_bytes())
        })
    }

    pub fn verify_sig_b64(&self, msg: &[u8], sig_b64: &str) -> bool {
        let vk = match &self.verifying { Some(v) => v, None => return false };
        let sig_bytes = match B64.decode(sig_b64) { Ok(b) => b, Err(_) => return false };
        let arr: [u8; 64] = match sig_bytes.as_slice().try_into() { Ok(a) => a, Err(_) => return false }; 
        let sig = Signature::from_bytes(&arr);
        vk.verify(msg, &sig).is_ok()
    }
}

impl Kernel {
    // --------------------------
    // JCS (RFC8785) + SHA-256
    // --------------------------
    pub fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    pub fn jcs_string<T: Serialize>(data: &T) -> String {
        jcs_to_string(data).expect("JCS serialization failed")
    }

    pub fn jcs_hash<T: Serialize>(data: &T) -> String {
        let s = Self::jcs_string(data);
        Self::sha256_hex(s.as_bytes())
    }

    pub fn now_rfc3339(meta: &ExecMeta) -> String {
        meta.execution_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))
    }

    fn time_bucket(ts: &str, unit: &str) -> String {
        let dt = match Self::parse_ts(ts) { Some(d) => d, None => return "".into() };
        match unit {
            "minute" => dt.format("%Y-%m-%dT%H:%M").to_string(),
            "hour" => dt.format("%Y-%m-%dT%H").to_string(),
            "day" => dt.format("%Y-%m-%d").to_string(),
            _ => "".into()
        }
    }

    // --------------------------
    // Path
    // --------------------------
    pub fn resolve_path(root: &Value, path: &[String]) -> Option<Value> {
        let mut curr = root;
        for key in path { curr = curr.get(key)?; }
        Some(curr.clone())
    }

    fn as_f64(v: &Value) -> Option<f64> { v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)) }

    // --------------------------
    // Expression eval (deterministic)
    // --------------------------
    pub fn eval_expr(expr: &Expr, ctx: &Value, meta: &ExecMeta) -> Value {
        match expr {
            Expr::Literal { value } => value.clone(),
            Expr::Path { path, fallback } => {
                Self::resolve_path(ctx, path).or_else(|| fallback.clone()).unwrap_or(Value::Null)
            }
            Expr::Compare { op, left, right } => {
                let l = Self::eval_expr(left, ctx, meta);
                let r = Self::eval_expr(right, ctx, meta);
                json!(Self::compare_strict(op, &l, &r))
            }
            Expr::Logic { op, args } => {
                let vals: Vec<bool> = args.iter()
                    .map(|a| Self::eval_expr(a, ctx, meta).as_bool().unwrap_or(false))
                    .collect();
                let res = match op {
                    LogicOp::And => vals.iter().all(|&x| x),
                    LogicOp::Or => vals.iter().any(|&x| x),
                    LogicOp::Not => !vals.first().copied().unwrap_or(false),
                };
                json!(res)
            }
            Expr::Call { function, args } => {
                let vals: Vec<Value> = args.iter().map(|a| Self::eval_expr(a, ctx, meta)).collect();
                match function.as_str() {
                    // time
                    "now" => json!(Self::now_rfc3339(meta)),
                    "before" => {
                        let a = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let b = vals.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let adt = Self::parse_ts(a);
                        let bdt = Self::parse_ts(b);
                        json!(adt.zip(bdt).map(|(x,y)| x < y).unwrap_or(false))
                    }
                    "after" => {
                        let a = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let b = vals.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let adt = Self::parse_ts(a);
                        let bdt = Self::parse_ts(b);
                        json!(adt.zip(bdt).map(|(x,y)| x > y).unwrap_or(false))
                    }
                    "age" => {
                        let a = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let adt = Self::parse_ts(a);
                        json!(adt.map(|x| (meta.execution_time - x).num_seconds()).unwrap_or(0))
                    }
                    "time_bucket" => {
                        let ts = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let unit = vals.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        json!(Self::time_bucket(ts, unit))
                    }

                    // string
                    "lower" => json!(vals.get(0).and_then(|v| v.as_str()).unwrap_or("").to_lowercase()),
                    "upper" => json!(vals.get(0).and_then(|v| v.as_str()).unwrap_or("").to_uppercase()),
                    "starts_with" => {
                        let s = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let p = vals.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        json!(s.starts_with(p))
                    }
                    "ends_with" => {
                        let s = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let p = vals.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        json!(s.ends_with(p))
                    }

                    // collections
                    "length" | "len" => {
                        if let Some(a) = vals.get(0).and_then(|v| v.as_array()) { json!(a.len()) }
                        else if let Some(s) = vals.get(0).and_then(|v| v.as_str()) { json!(s.chars().count()) }
                        else { json!(0) }
                    }
                    "empty" => json!(vals.get(0).and_then(|v| v.as_array()).map(|a| a.is_empty()).unwrap_or(true)),
                    "contains" => {
                        if let (Some(s), Some(sub)) = (vals.get(0).and_then(|v| v.as_str()), vals.get(1).and_then(|v| v.as_str())) {
                            json!(s.contains(sub))
                        } else if let Some(arr) = vals.get(0).and_then(|v| v.as_array()) {
                            json!(arr.contains(vals.get(1).unwrap_or(&Value::Null)))
                        } else { json!(false) }
                    }

                    // numbers
                    "abs" => json!(Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).map(|n| n.abs()).unwrap_or(0.0)),
                    "floor" => json!(Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).map(|n| n.floor()).unwrap_or(0.0)),
                    "ceil" => json!(Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).map(|n| n.ceil()).unwrap_or(0.0)),
                    "min" => {
                        let a = Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        let b = Self::as_f64(vals.get(1).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        json!(a.min(b))
                    }
                    "max" => {
                        let a = Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        let b = Self::as_f64(vals.get(1).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        json!(a.max(b))
                    }
                    "add" => {
                        let a = Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        let b = Self::as_f64(vals.get(1).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        json!(a + b)
                    }
                    "sub" => {
                        let a = Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        let b = Self::as_f64(vals.get(1).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        json!(a - b)
                    }
                    "div" => {
                        let a = Self::as_f64(vals.get(0).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        let b = Self::as_f64(vals.get(1).unwrap_or(&Value::Null)).unwrap_or(0.0);
                        if b == 0.0 { json!(0.0) } else { json!(a / b) }
                    }

                    // crypto
                    "sha256" => {
                        let s = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        json!(Self::sha256_hex(s.as_bytes()))
                    }
                    "verify_ed25519" => {
                        let pk_b64 = vals.get(0).and_then(|v| v.as_str()).unwrap_or("");
                        let msg = vals.get(1).and_then(|v| v.as_str()).unwrap_or("");
                        let sig_b64 = vals.get(2).and_then(|v| v.as_str()).unwrap_or("");

                        let pk_bytes = match B64.decode(pk_b64) { Ok(b) => b, Err(_) => return json!(false) };
                        let arr: [u8; 32] = match pk_bytes.as_slice().try_into() { Ok(a) => a, Err(_) => return json!(false) };
                        let vk = match VerifyingKey::from_bytes(&arr) { Ok(v) => v, Err(_) => return json!(false) };

                        let sig_bytes = match B64.decode(sig_b64) { Ok(b) => b, Err(_) => return json!(false) };
                        let arr: [u8; 64] = match sig_bytes.as_slice().try_into() { Ok(a) => a, Err(_) => return json!(false) }; 
                        let sig = Signature::from_bytes(&arr);

                        json!(vk.verify(msg.as_bytes(), &sig).is_ok())
                    }

                    _ => Value::Null
                }
            }
        }
    }

    fn compare_strict(op: &CompareOp, l: &Value, r: &Value) -> bool {
        match op {
            CompareOp::Eq => l == r,
            CompareOp::Ne => l != r,
            CompareOp::Exists => !l.is_null(),
            CompareOp::In => {
                if let Some(arr) = r.as_array() { arr.contains(l) }
                else if let (Some(ls), Some(rs)) = (l.as_str(), r.as_str()) { rs.contains(ls) }
                else { false }
            }
            CompareOp::Gt | CompareOp::Lt | CompareOp::Ge | CompareOp::Le => {
                if let (Some(a), Some(b)) = (Self::as_f64(l), Self::as_f64(r)) {
                    match op {
                        CompareOp::Gt => a > b,
                        CompareOp::Lt => a < b,
                        CompareOp::Ge => a >= b,
                        CompareOp::Le => a <= b,
                        _ => false
                    }
                } else { false }
            }
        }
    }

    // --------------------------
    // Content-addressed hashes
    // --------------------------
    pub fn compute_chip_hash(chip: &Chip) -> String {
        let mut tmp = chip.clone(); tmp.hash = "".into();
        Self::jcs_hash(&tmp)
    }

    pub fn compute_program_hash(program: &Program) -> String {
        let mut tmp = program.clone(); tmp.hash = "".into();
        Self::jcs_hash(&tmp)
    }

    // --------------------------
    // Gate evaluation with evidence
    // --------------------------
    fn eval_gate_expr(expr: &Expr, ctx: &Value, meta: &ExecMeta) -> (bool, GateValues, Option<String>) {
        match expr {
            Expr::Compare { op, left, right } => {
                let l = Self::eval_expr(left, ctx, meta);
                let r = Self::eval_expr(right, ctx, meta);
                let ok = Self::compare_strict(op, &l, &r);
                (ok, GateValues { left: Some(l), right: Some(r) }, None)
            }
            _ => {
                let v = Self::eval_expr(expr, ctx, meta);
                match v.as_bool() {
                    Some(b) => (b, GateValues::default(), None),
                    None => (false, GateValues::default(), Some("gate_not_boolean".into())),
                }
            }
        }
    }

    // --------------------------
    // Chip execution -> Proof (+ optional signature)
    // --------------------------
    pub fn execute_chip_signed(chip: &Chip, ctx: &Value, meta: &ExecMeta, keys: &KeyMaterial) -> Proof {
        let mut gates: Vec<GateResult> = vec![];
        for g in &chip.gates {
            let (result, values, error) = Self::eval_gate_expr(&g.expr, ctx, meta);
            gates.push(GateResult { id: g.id.clone(), result, values, error });
        }

        let passed = gates.iter().filter(|g| g.result).count();
        let total = gates.len().max(1);

        let comp = match &chip.composition {
            Composition::Shorthand(s) => {
                let kind = match s.as_str() {
                    "ALL" => CompositionType::ALL,
                    "ANY" => CompositionType::ANY,
                    "MAJORITY" => CompositionType::MAJORITY,
                    "WEIGHTED" => CompositionType::WEIGHTED,
                    _ => CompositionType::ALL
                };
                CompositionDef { kind, weights: vec![], threshold: 0.0 }
            }
            Composition::Full(c) => c.clone()
        };

        let final_result: u8 = match comp.kind {
            CompositionType::ALL => if passed == total { 1 } else { 0 },
            CompositionType::ANY => if passed > 0 { 1 } else { 0 },
            CompositionType::MAJORITY => if passed * 2 > total { 1 } else { 0 },
            CompositionType::WEIGHTED => {
                if comp.weights.len() != chip.gates.len() { 0 }
                else {
                    let mut sum = 0.0;
                    for (i, gr) in gates.iter().enumerate() {
                        if gr.result { sum += comp.weights[i]; }
                    }
                    if sum > comp.threshold { 1 } else { 0 }
                }
            }
        };

        let failed_gates: Vec<String> = gates.iter().filter(|g| !g.result).map(|g| g.id.clone()).collect();

        let mut proof = Proof {
            chip_hash: chip.hash.clone(),
            evaluated_at: Self::now_rfc3339(meta),
            context_snapshot: ctx.clone(),
            gates,
            failed_gates,
            final_result,
            proof_hash: "".into(),
            signature: None,
        };

        // proof_hash excludes signature + proof_hash itself
        let mut tmp = proof.clone();
        tmp.proof_hash = "".into();
        tmp.signature = None;
        proof.proof_hash = Self::jcs_hash(&tmp);

        if let Some(sig) = keys.sign_b64(proof.proof_hash.as_bytes()) {
            proof.signature = Some(sig);
        }

        proof
    }

    // --------------------------
    // Proof verification (chip + snapshot + signature)
    // --------------------------
    pub fn verify_proof(proof: &Proof, chip: &Chip, keys: &KeyMaterial) -> bool {
        // chip hash
        if proof.chip_hash != chip.hash { return false; }

        // recompute proof_hash (exclude signature/proof_hash)
        let mut tmp = proof.clone();
        tmp.proof_hash = "".into();
        tmp.signature = None;
        let recomputed = Self::jcs_hash(&tmp);
        if recomputed != proof.proof_hash { return false; }

        // deterministic re-exec at same evaluated_at time
        let exec_time = DateTime::parse_from_rfc3339(&proof.evaluated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let meta = ExecMeta { tx_id: "verify".into(), execution_time: exec_time };
        let check = Self::execute_chip_signed(chip, &proof.context_snapshot, &meta, &KeyMaterial { signing: None, verifying: None });
        if check.final_result != proof.final_result { return false; }

        // signature verify if present and verifying key exists
        if let (Some(sig_b64), true) = (proof.signature.as_deref(), keys.verifying.is_some()) {
            if !keys.verify_sig_b64(proof.proof_hash.as_bytes(), sig_b64) { return false; }
        }

        true
    }
}
