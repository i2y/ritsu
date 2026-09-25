//! dandori: a small typed language for workflows that call business rules. A `.flow` is
//! checked before it runs — types, every arm of every match, every state a case can be
//! left in — and compiled to AWS Step Functions (ASL with JSONata), to Temporal
//! (TypeScript) and to AWS Lambda durable functions (TypeScript). The rules themselves are
//! written in rulec and read through its CLI.

pub mod asl;
pub mod aws;
pub mod check;
pub mod diag;
pub mod flow;
pub mod interp;
pub mod lower;
pub mod model;
pub mod render;
pub mod rulec;
pub mod scenarios;
pub mod syntax;
pub mod temporal;
