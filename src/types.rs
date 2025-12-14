use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Hash = String;
pub type Timestamp = String;

// ----------------------
// Expressions
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Expr {
    Literal { value: Value },
    Path { path: Vec<String>, #[serde(default)] fallback: Option<Value> },
    Compare { op: CompareOp, left: Box<Expr>, right: Box<Expr> },
    Logic { op: LogicOp, args: Vec<Expr> },
    Call { function: String, args: Vec<Expr> },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CompareOp {
    #[serde(rename="==")] Eq,
    #[serde(rename="!=")] Ne,
    #[serde(rename=">")] Gt,
    #[serde(rename="<")] Lt,
    #[serde(rename=">=")] Ge,
    #[serde(rename="<=")] Le,
    #[serde(rename="in")] In,
    #[serde(rename="exists")] Exists,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum LogicOp { And, Or, Not }

// ----------------------
// Chip
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Gate {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub expr: Expr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Composition {
    Shorthand(String),        // "ALL" | "ANY" | "MAJORITY" | "WEIGHTED"
    Full(CompositionDef),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompositionDef {
    #[serde(rename="type")]
    pub kind: CompositionType,
    #[serde(default)]
    pub weights: Vec<f64>,
    #[serde(default)]
    pub threshold: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum CompositionType { ALL, ANY, MAJORITY, WEIGHTED }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Chip {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub gates: Vec<Gate>,
    #[serde(default)]
    pub composition: Composition,
    #[serde(default)]
    pub hash: Hash,
}

// ----------------------
// Program
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProgramInput {
    pub name: String,
    #[serde(rename="type")]
    pub input_type: String,
    pub required: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextDef {
    pub name: String,
    pub source: ContextSource,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub expression: Option<Expr>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource { Ledger, Input, Computed }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Program {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub inputs: Vec<ProgramInput>,
    pub context: Vec<ContextDef>,
    pub evaluate: Hash,
    pub on_allow: Vec<Effect>,
    pub on_deny: Vec<Effect>,
    #[serde(default)]
    pub hash: Hash,
}

// ----------------------
// Effects
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag="type", rename_all="snake_case")]
pub enum Effect {
    Set { target: String, value: Expr },
    Increment { target: String, amount: Expr },
    Decrement { target: String, amount: Expr },
    Append { target: String, value: Expr },
    Remove { target: String, value: Expr },
    Create { entity_type: String, id: Expr, data: Value },
    Delete { target: String },
    Emit { event: String, data: Value },
    Fail { message: String },
}

// ----------------------
// Proof
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proof {
    pub chip_hash: Hash,
    pub evaluated_at: Timestamp,
    pub context_snapshot: Value,
    pub gates: Vec<GateResult>,
    pub failed_gates: Vec<String>,
    pub final_result: u8,   // 0|1
    pub proof_hash: Hash,
    #[serde(default)]
    pub signature: Option<String>, // base64(ed25519(sig(proof_hash bytes)))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GateResult {
    pub id: String,
    pub result: bool,
    #[serde(default)]
    pub values: GateValues,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GateValues {
    #[serde(default)]
    pub left: Option<Value>,
    #[serde(default)]
    pub right: Option<Value>,
}

// ----------------------
// EffectRecord
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EffectRecord {
    pub id: String,
    pub version_applied_to: u64,
    pub resulting_version: u64,
    pub timestamp: Timestamp,
    pub program_hash: Hash,
    pub input_hash: Hash,
    pub proof_hash: Hash,
    pub applied_effects: Vec<Effect>,
    #[serde(default)]
    pub previous_record_hash: Option<Hash>,
    pub record_hash: Hash,
    #[serde(default)]
    pub record_signature: Option<String>, // base64(ed25519(sig(record_hash bytes)))
}

// ----------------------
// API
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecReq {
    pub program: String,
    pub inputs: Value,
    #[serde(default)]
    pub target_version: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag="type", rename_all="snake_case")]
pub enum RegisterReq {
    Chip { data: Chip },
    Program { data: Program },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VerifyReq {
    pub proof: Proof,
}

// ----------------------
// Trust / Barrier
// ----------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all="snake_case")]
pub enum ContentType {
    Invoice,
    Email,
    Contract,
    ApiResponse,
    UserInput,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BarrierReq {
    pub content_type: ContentType,
    pub payload: Value,
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValidatedData {
    pub content_type: ContentType,
    pub fields: Value,
    pub content_hash: String,
    #[serde(default)]
    pub signature: Option<String>,
}
