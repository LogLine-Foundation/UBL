use crate::error::UblError;
use crate::engine::{Kernel, ExecMeta, KeyMaterial};
use crate::interp;
use crate::types::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs::File, path::Path, sync::Arc};
use tracing::info;

const DB_FILE: &str = "ubl_ledger.json";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LedgerState {
    pub meta: Meta,
    pub registry: Registry,
    pub root: Value,              // entity tree
    pub history: Vec<EffectRecord>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Meta {
    pub version: u64,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Registry {
    pub chips: std::collections::HashMap<String, Chip>,
    #[serde(default)]
    pub chip_names: std::collections::HashMap<String, String>, // name -> hash
    pub programs: std::collections::HashMap<String, Program>,
}

pub struct Ledger {
    state: Arc<RwLock<LedgerState>>,
}

impl Ledger {
    pub fn new() -> Self {
        let state = if Path::new(DB_FILE).exists() {
            let content = std::fs::read_to_string(DB_FILE).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            LedgerState {
                meta: Meta { version: 0, created_at: chrono::Utc::now().to_rfc3339() },
                root: json!({}),
                ..Default::default()
            }
        };
        info!("📚 Ledger Mounted. Version: {}", state.meta.version);
        Self { state: Arc::new(RwLock::new(state)) }
    }

    pub fn snapshot_root(&self) -> Value {
        self.state.read().root.clone()
    }

    pub fn current_version(&self) -> u64 {
        self.state.read().meta.version
    }

    pub fn get_program(&self, name: &str) -> Option<Program> {
        self.state.read().registry.programs.get(name).cloned()
    }

    pub fn get_chip(&self, hash: &str) -> Option<Chip> {
        self.state.read().registry.chips.get(hash).cloned()
    }

    pub fn get_chip_by_name(&self, name: &str) -> Option<Chip> {
        let st = self.state.read();
        let h = st.registry.chip_names.get(name)?;
        st.registry.chips.get(h).cloned()
    }

    pub fn list_chips(&self) -> Vec<(String, String, String)> {
        self.state.read().registry.chips.iter()
            .map(|(h, c)| (h.clone(), c.name.clone(), c.description.clone()))
            .collect()
    }

    pub fn list_programs(&self) -> Vec<(String, String)> {
        self.state.read().registry.programs.iter()
            .map(|(n, p)| (n.clone(), p.hash.clone()))
            .collect()
    }

    pub fn register_chip(&self, mut chip: Chip) -> Result<String, UblError> {
        let computed = Kernel::compute_chip_hash(&chip);
        chip.hash = computed.clone();
        let mut st = self.state.write();

        // Enforce unique chip name -> hash mapping (unless identical logic).
        if let Some(existing) = st.registry.chip_names.get(&chip.name) {
            if existing != &computed {
                return Err(UblError::Validation(format!(
                    "chip_name_conflict: name='{}' existing_hash='{}' new_hash='{}'",
                    chip.name, existing, computed
                )));
            }
        }

        st.registry.chip_names.insert(chip.name.clone(), computed.clone());
        st.registry.chips.insert(computed.clone(), chip);
        Ok(computed)
    }

    pub fn register_program(&self, mut program: Program) -> Result<String, UblError> {
        let computed = Kernel::compute_program_hash(&program);
        program.hash = computed.clone();
        self.state.write().registry.programs.insert(program.name.clone(), program);
        Ok(computed)
    }

    pub async fn commit(&self) -> Result<(), UblError> {
        let snapshot = { self.state.read().clone() };
        let json_str = serde_json::to_string_pretty(&snapshot)?;

        let tmp_file = format!("{}.tmp", DB_FILE);
        tokio::fs::write(&tmp_file, json_str).await.map_err(|e| UblError::LedgerIo(e.to_string()))?;

        { // fsync tmp
            let f = File::open(&tmp_file).map_err(|e| UblError::LedgerIo(e.to_string()))?;
            f.sync_all().map_err(|e| UblError::LedgerIo(e.to_string()))?;
        }

        tokio::fs::rename(&tmp_file, DB_FILE).await.map_err(|e| UblError::LedgerIo(e.to_string()))?;

        if let Some(parent) = Path::new(DB_FILE).parent() {
            if let Ok(dir) = File::open(parent) { let _ = dir.sync_all(); }
        }
        Ok(())
    }

    // --------------------------
    // Apply effects atomically
    // --------------------------
    pub async fn apply_transaction(
        &self,
        program_hash: &str,
        input_hash: &str,
        target_version: Option<u64>,
        proof: &Proof,
        effects: &[Effect],
        meta: &ExecMeta,
        keys: &KeyMaterial,
    ) -> Result<EffectRecord, UblError> {
        let mut st = self.state.write();
        let v = st.meta.version;

        if let Some(tv) = target_version {
            if tv != v {
                return Err(UblError::Validation(format!("version_conflict: expected {}, got {}", tv, v)));
            }
        }

        let mut root = st.root.clone();
        let mut applied: Vec<Effect> = vec![];

        for eff in effects {
            match eff {
                Effect::Fail { message } => return Err(UblError::Validation(format!("program_fail: {}", message))),
                Effect::Emit { event, data } => {
                    // Resolve templated strings inside event payloads for a fully replayable EffectRecord.
                    let ev = interp::interpolate_str(event, &proof.context_snapshot, Some(proof), meta);
                    let d  = interp::interpolate_value(data, &proof.context_snapshot, Some(proof), meta);
                    applied.push(Effect::Emit { event: ev, data: d });
                }
                Effect::Create { entity_type, id, data } => {
                    let idv = Kernel::eval_expr(id, &proof.context_snapshot, meta);
                    let id_str = idv.as_str().map(|s| s.to_string()).unwrap_or_else(|| idv.to_string());

                    if root.get(entity_type).and_then(|c| c.get(&id_str)).is_some() {
                        return Err(UblError::Validation(format!("entity_exists: {}.{}", entity_type, id_str)));
                    }

                    let resolved_data = interp::interpolate_value(data, &proof.context_snapshot, Some(proof), meta);

                    ensure_obj_path(&mut root, &[entity_type.as_str()])?;
                    if let Some(coll) = root.get_mut(entity_type).and_then(|v| v.as_object_mut()) {
                        coll.insert(id_str.clone(), resolved_data.clone());
                    }

                    applied.push(Effect::Create {
                        entity_type: entity_type.clone(),
                        id: lit(json!(id_str)),
                        data: resolved_data,
                    });
                }
                Effect::Delete { target } => {
                    let t = interp::interpolate_str(target, &proof.context_snapshot, Some(proof), meta);
                    delete_path(&mut root, &t)?;
                    applied.push(Effect::Delete { target: t });
                }
                Effect::Set { target, value } => {
                    let t = interp::interpolate_str(target, &proof.context_snapshot, Some(proof), meta);
                    let raw = Kernel::eval_expr(value, &proof.context_snapshot, meta);
                    let v = interp::interpolate_value(&raw, &proof.context_snapshot, Some(proof), meta);
                    set_path(&mut root, &t, v.clone())?;
                    applied.push(Effect::Set { target: t, value: lit(v) });
                }
                Effect::Increment { target, amount } => {
                    let t = interp::interpolate_str(target, &proof.context_snapshot, Some(proof), meta);
                    let a_val = Kernel::eval_expr(amount, &proof.context_snapshot, meta);
                    let a_val = interp::interpolate_value(&a_val, &proof.context_snapshot, Some(proof), meta);
                    let a = a_val.as_f64().unwrap_or(0.0);
                    let curr = get_path(&root, &t).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    set_path(&mut root, &t, json!(curr + a))?;
                    applied.push(Effect::Increment { target: t, amount: lit(json!(a)) });
                }
                Effect::Decrement { target, amount } => {
                    let t = interp::interpolate_str(target, &proof.context_snapshot, Some(proof), meta);
                    let a_val = Kernel::eval_expr(amount, &proof.context_snapshot, meta);
                    let a_val = interp::interpolate_value(&a_val, &proof.context_snapshot, Some(proof), meta);
                    let a = a_val.as_f64().unwrap_or(0.0);
                    let curr = get_path(&root, &t).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    set_path(&mut root, &t, json!(curr - a))?;
                    applied.push(Effect::Decrement { target: t, amount: lit(json!(a)) });
                }
                Effect::Append { target, value } => {
                    let t = interp::interpolate_str(target, &proof.context_snapshot, Some(proof), meta);
                    let raw = Kernel::eval_expr(value, &proof.context_snapshot, meta);
                    let v = interp::interpolate_value(&raw, &proof.context_snapshot, Some(proof), meta);
                    let mut arr = get_path(&root, &t).and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    arr.push(v.clone());
                    set_path(&mut root, &t, Value::Array(arr))?;
                    applied.push(Effect::Append { target: t, value: lit(v) });
                }
                Effect::Remove { target, value } => {
                    let t = interp::interpolate_str(target, &proof.context_snapshot, Some(proof), meta);
                    let raw = Kernel::eval_expr(value, &proof.context_snapshot, meta);
                    let v = interp::interpolate_value(&raw, &proof.context_snapshot, Some(proof), meta);
                    let mut arr = get_path(&root, &t).and_then(|v| v.as_array().cloned()).unwrap_or_default();
                    arr.retain(|x| x != &v);
                    set_path(&mut root, &t, Value::Array(arr))?;
                    applied.push(Effect::Remove { target: t, value: lit(v) });
                }
            }
        }

        let prev_hash = st.history.last().map(|r| r.record_hash.clone());
        let new_version = v + 1;

        let mut record = EffectRecord {
            id: meta.tx_id.clone(),
            version_applied_to: v,
            resulting_version: new_version,
            timestamp: Kernel::now_rfc3339(meta),
            program_hash: program_hash.to_string(),
            input_hash: input_hash.to_string(),
            proof_hash: proof.proof_hash.clone(),
            applied_effects: applied,
            previous_record_hash: prev_hash,
            record_hash: "".into(),
            record_signature: None,
        };

        let mut tmp = record.clone();
        tmp.record_hash = "".into();
        tmp.record_signature = None;
        record.record_hash = Kernel::jcs_hash(&tmp);

        // Optional signature over record_hash
        if let Some(sig) = keys.sign_b64(record.record_hash.as_bytes()) {
            record.record_signature = Some(sig);
        }

        st.root = root;
        st.meta.version = new_version;
        st.history.push(record.clone());
        drop(st);

        self.commit().await?;
        Ok(record)
    }
}

// --------------------------
// JSON path helpers
// --------------------------
fn split_path(path: &str) -> Vec<&str> { path.split('.').filter(|s| !s.is_empty()).collect() }

fn lit(value: Value) -> Expr {
    Expr::Literal { value }
}

fn ensure_obj_path(root: &mut Value, parts: &[&str]) -> Result<(), UblError> {
    let mut cur = root;
    for p in parts {
        if cur.get(*p).is_none() {
            if let Some(obj) = cur.as_object_mut() { obj.insert(p.to_string(), json!({})); }
            else { return Err(UblError::State("ensure_obj_path_non_object".into())); }
        }
        cur = cur.get_mut(*p).ok_or_else(|| UblError::State("ensure_obj_path_failed".into()))?;
        if !cur.is_object() { *cur = json!({}); }
    }
    Ok(())
}

fn get_path(root: &Value, path: &str) -> Option<Value> {
    let parts = split_path(path);
    let mut cur = root;
    for p in parts { cur = cur.get(p)?; }
    Some(cur.clone())
}

fn set_path(root: &mut Value, path: &str, val: Value) -> Result<(), UblError> {
    let parts = split_path(path);
    if parts.is_empty() { return Err(UblError::State("empty_path".into())); }
    let mut cur = root;
    for (i, p) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Some(obj) = cur.as_object_mut() {
                obj.insert((*p).to_string(), val);
                return Ok(());
            } else {
                return Err(UblError::State("set_path_non_object".into()));
            }
        }
        if cur.get(*p).is_none() {
            if let Some(obj) = cur.as_object_mut() { obj.insert((*p).to_string(), json!({})); }
            else { return Err(UblError::State("set_path_non_object".into())); }
        }
        cur = cur.get_mut(*p).ok_or_else(|| UblError::State("invalid_path".into()))?;
        if !cur.is_object() { *cur = json!({}); }
    }
    Ok(())
}

fn delete_path(root: &mut Value, path: &str) -> Result<(), UblError> {
    let parts = split_path(path);
    if parts.is_empty() { return Ok(()); }
    if parts.len() == 1 {
        if let Some(obj) = root.as_object_mut() { obj.remove(parts[0]); }
        return Ok(());
    }
    let mut cur = root;
    for p in &parts[..parts.len()-1] {
        cur = match cur.get_mut(*p) { Some(v) => v, None => return Ok(()) };
    }
    if let Some(obj) = cur.as_object_mut() { obj.remove(parts[parts.len()-1]); }
    Ok(())
}

// NOTE: Interpolation for templated strings is implemented in `src/interp.rs`.
