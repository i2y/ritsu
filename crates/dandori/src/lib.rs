//! dandori: a small typed language for workflows that call business rules. A `.flow` is
//! checked before it runs — types, every arm of every match, every state a case can be
//! left in — and compiled to AWS Step Functions (ASL with JSONata), to Temporal
//! (TypeScript or Python), to AWS Lambda durable functions (TypeScript), to Argo Workflows
//! (a WorkflowTemplate) and to pydantic-graph (Python). The rules themselves are written in rulec and read through its CLI.

pub mod argo;
pub mod asl;
pub mod aws;
pub mod check;
pub mod diag;
pub mod flow;
pub mod interp;
pub mod lower;
pub mod model;
pub mod pydantic_graph;
pub mod render;
pub mod rulec;
pub mod scenarios;
pub mod syntax;
pub mod temporal;
pub mod temporal_py;
