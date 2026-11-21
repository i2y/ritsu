//! ringi (稟議): a small language for a company's applications and approvals. A `.ringi` holds a
//! kind of request: its form, the route it takes and who approves at each stage, its deadlines, and
//! what happens when it ends; or what several kinds share: the shape of the organization, the lists
//! a form chooses from, a route. The design is DESIGN.md, and the stages it is built in are
//! PLAN.md.
//!
//! The parts, in the order a file goes through them:
//!
//! - [`lex`] and [`parse`]: a `.ringi` into its syntax tree ([`ast`]), with the words of [`kw`].
//! - [`names`]: the files its `use` lines read (the organization, the masters and the shared routes
//!   ringi reads itself; the files of the other languages are asked through the ports of
//!   [`suite`]), and the names it declares and refers to.
//! - [`types`]: the type of every expression and condition; [`form`], the ranges, lengths, rounding
//!   and the order in which a form's fields are filled; [`route`], the stages, the quorums and the
//!   ways to choose an approver.
//! - [`lower`]: a request lowered to the files of rulec and koyomi that check it, each line with
//!   the line of the request it comes from; [`mapped`], those files checked by their languages and
//!   what they say said again at the request's lines; [`claims`], what ringi asks rulec of them
//!   itself (a field hidden while it is required, a stage that only some requests reach).
//! - [`org`]: the data of an organization (its versions, people, teams and proxies) and of masters,
//!   read when their files are checked; who answers each stage of a request in each version, and
//!   what is said of it. [`usable`]: the days a request can be used on, which the data decides.
//! - [`check`]: a file from its text to its diagnostics ([`diag`], the codes of [`codes`]), and a
//!   key written in the file looked for in its whole text ([`security`]).
//! - [`simulate`]: who answers each stage of a request for a requester, the values of its form and
//!   a day.
//! - [`engine`]: the ticket machine, the reference interpreter of a request: a ticket made from a
//!   `submit` and moved by one event at a time, with the things to do. [`taken`]: what a form takes,
//!   decided once for the check, `simulate` and the machine. [`ends`]: that every ticket ends, the
//!   most business days one takes, and that the holds it opens are closed.
//! - [`scenario`]: the scenarios of a request run on the machine (`run`). [`vectors`]: transitions
//!   of the machine written out for the generated code and a server to answer the same.
//! - [`r#gen`]: what `ritsu gen --target requests` writes of the requests apart from any language:
//!   the description of each kind, the server's API, the tools of an agent, the schema of the
//!   organization's data, the vectors. [`find`]: how a person, a team or a row of a master is found
//!   by words, the reference of the server's find, and its vectors.
//! - [`doc`]: the pages of `doc`, for the people who approve and manage the requests.
//! - [`ticket`]: what the checks across the borders and `api` read of a request's ticket: its
//!   operations, how long a round takes, its holds, and the flows it starts and asks with what
//!   each is given.
//! - [`cli`] and [`run`]: the `ringi` command, which the binary runs and so does `ritsu ringi`.
//! - [`ports`]: ringi as `ritsu check` joins it.

#[macro_use]
extern crate ritsu_base;

pub mod apart;
pub mod api;
pub mod ast;
pub mod check;
pub mod claims;
pub mod cli;
pub mod codes;
pub mod diag;
pub mod diff;
pub mod doc;
pub mod ends;
pub mod engine;
pub mod find;
pub mod form;
/// What `ritsu gen --target requests` writes: the kinds of request apart from any language.
pub mod r#gen;
pub mod items;
pub mod kw;
pub mod lex;
pub mod lower;
pub mod mapped;
pub mod model;
pub mod names;
pub mod org;
pub mod parse;
pub mod ports;
pub mod route;
/// The `ringi` command, as a function: the binary runs it, and so does `ritsu ringi`.
pub mod run;
pub mod scenario;
pub mod screens;
pub mod security;
pub mod simulate;
pub mod suite;
pub mod taken;
pub mod ticket;
pub mod types;
pub mod usable;
pub mod vectors;
