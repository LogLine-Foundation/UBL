use crate::error::UblError;
use crate::engine::Kernel;
use crate::types::{BarrierReq, ContentType, ValidatedData};
use serde_json::{json, Value};

fn expect_string(v: &Value, field: &str) -> Result<String, UblError> {
    v.as_str().map(|s| s.to_string()).ok_or_else(|| UblError::Validation(format!("type_mismatch: {} expected string", field)))
}
fn expect_number(v: &Value, field: &str) -> Result<f64, UblError> {
    v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)).ok_or_else(|| UblError::Validation(format!("type_mismatch: {} expected number", field)))
}

pub fn process(req: &BarrierReq) -> Result<ValidatedData, UblError> {
    let payload = req.payload.as_object().ok_or_else(|| UblError::Validation("payload_must_be_object".into()))?;

    let fields = match req.content_type {
        ContentType::Invoice => {
            // required: vendor_id, amount, currency, date
            let vendor_id = payload.get("vendor_id").ok_or_else(|| UblError::Validation("missing: vendor_id".into()))?;
            let amount = payload.get("amount").ok_or_else(|| UblError::Validation("missing: amount".into()))?;
            let currency = payload.get("currency").ok_or_else(|| UblError::Validation("missing: currency".into()))?;
            let date = payload.get("date").ok_or_else(|| UblError::Validation("missing: date".into()))?;

            let mut out = serde_json::Map::new();
            out.insert("vendor_id".into(), json!(expect_string(vendor_id, "vendor_id")?));
            out.insert("amount".into(), json!(expect_number(amount, "amount")?));
            out.insert("currency".into(), json!(expect_string(currency, "currency")?));
            out.insert("date".into(), json!(expect_string(date, "date")?));

            // optional: description, line_items, reference
            if let Some(d) = payload.get("description") {
                if d.is_string() { out.insert("description".into(), d.clone()); }
            }
            if let Some(li) = payload.get("line_items") {
                if li.is_array() { out.insert("line_items".into(), li.clone()); }
            }
            if let Some(r) = payload.get("reference") {
                if r.is_string() { out.insert("reference".into(), r.clone()); }
            }

            Value::Object(out)
        }
        ContentType::Email => {
            // required: from,to,subject,body
            let from = payload.get("from").ok_or_else(|| UblError::Validation("missing: from".into()))?;
            let to = payload.get("to").ok_or_else(|| UblError::Validation("missing: to".into()))?;
            let subject = payload.get("subject").ok_or_else(|| UblError::Validation("missing: subject".into()))?;
            let body = payload.get("body").ok_or_else(|| UblError::Validation("missing: body".into()))?;

            let mut out = serde_json::Map::new();
            out.insert("from".into(), json!(expect_string(from, "from")?));
            out.insert("to".into(), json!(expect_string(to, "to")?));
            out.insert("subject".into(), json!(expect_string(subject, "subject")?));
            out.insert("body".into(), json!(expect_string(body, "body")?));

            // optional: cc, attachments, timestamp
            if let Some(cc) = payload.get("cc") { if cc.is_array() { out.insert("cc".into(), cc.clone()); } }
            if let Some(att) = payload.get("attachments") { if att.is_array() { out.insert("attachments".into(), att.clone()); } }
            if let Some(ts) = payload.get("timestamp") { if ts.is_string() { out.insert("timestamp".into(), ts.clone()); } }

            Value::Object(out)
        }
        _ => {
            // Pass-through but still require object; drop nothing (caller chooses schema)
            Value::Object(payload.clone())
        }
    };

    // content_hash = sha256(JCS(payload))
    let jcs = Kernel::jcs_string(&req.payload);
    let content_hash = Kernel::sha256_hex(jcs.as_bytes());

    Ok(ValidatedData {
        content_type: req.content_type.clone(),
        fields,
        content_hash,
        signature: req.signature.clone(),
    })
}
