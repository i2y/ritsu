//! dandori: a small typed language for workflows that call business rules. A `.flow` is
//! checked before it runs — types, every arm of every match, every state a case can be
//! left in — and compiled to AWS Step Functions (ASL with JSONata), to Temporal
//! (TypeScript or Python), to AWS Lambda durable functions (TypeScript), to Argo Workflows
//! (a WorkflowTemplate) and to pydantic-graph (Python). The rules themselves are written in rulec and
//! read through ritsu's port of rules, which whoever runs dandori hands it (`cli::run`; sources).
//! Compiled to wasm32, the same library runs in ritsu's playground (crates/ritsu-wasm).

#[macro_use]
extern crate ritsu_base;

pub mod apis;
pub mod argo;
pub mod asl;
pub mod aws;
pub mod check;
pub mod codes;
pub mod cli;
pub mod commands;
pub mod computed;
pub mod contract;
pub mod crossings;
pub mod diag;
pub mod doc;
pub mod draw;
pub mod flow;
pub mod interp;
pub mod lower;
pub mod model;
pub mod prechecks;
pub mod ports;
pub mod proto;
pub mod pydantic_graph;
pub mod ranges;
pub mod render;
pub mod rulec;
pub mod scenarios;
pub mod secrets;
pub mod service;
pub mod sources;
pub mod syntax;
pub mod temporal;
pub mod temporal_go;
pub mod temporal_py;
