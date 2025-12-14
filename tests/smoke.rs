use ubl_core::engine::{Kernel, ExecMeta, KeyMaterial};
use ubl_core::types::*;
use serde_json::json;

#[test]
fn jcs_hash_is_deterministic() {
    let a = json!({"b":1,"a":2});
    let b = json!({"a":2,"b":1});
    assert_eq!(Kernel::jcs_hash(&a), Kernel::jcs_hash(&b));
}

#[test]
fn barrier_drops_unknown_fields() {
    let req = BarrierReq {
        content_type: ContentType::Invoice,
        payload: json!({"vendor_id":"v","amount":1,"currency":"USD","date":"2025-01-01","extra":"x"}),
        signature: None,
    };
    let out = ubl_core::trust_barrier::process(&req).unwrap();
    assert!(out.fields.get("extra").is_none());
    assert_eq!(out.fields.get("vendor_id").unwrap(), "v");
}

#[test]
fn proof_hash_recomputes() {
    // Minimal chip: amount > 0
    let chip = Chip {
        name: "p".into(),
        description: "".into(),
        gates: vec![Gate {
            id: "g".into(),
            description: "".into(),
            expr: Expr::Compare {
                op: CompareOp::Gt,
                left: Box::new(Expr::Path { path: vec!["amount".into()], fallback: None }),
                right: Box::new(Expr::Literal { value: json!(0) }),
            },
        }],
        composition: Composition::Shorthand("ALL".into()),
        hash: "".into(),
    };
    let mut chip2 = chip.clone();
    chip2.hash = Kernel::compute_chip_hash(&chip2);

    let meta = ExecMeta { tx_id: "t".into(), execution_time: chrono::Utc::now() };
    let ctx = json!({"amount": 1});
    let proof = Kernel::execute_chip_signed(&chip2, &ctx, &meta, &KeyMaterial { signing: None, verifying: None });

    let ok = Kernel::verify_proof(&proof, &chip2, &KeyMaterial { signing: None, verifying: None });
    assert!(ok);
}
