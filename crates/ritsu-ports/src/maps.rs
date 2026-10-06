//! The port of maps (DESIGN 16.8, 16.9): sakai's maps as the checks across the borders read them —
//! the contexts of a map, the relationships between them, and the context a file of the project
//! belongs to, each decided by sakai's own rules. ritsu-cross holds the secrets a flow sends
//! (`Flows::sends`) to them (X14): a secret may go to its own context, or to one the map relates
//! to it, and to no file outside the map. It holds the operations each context opens to the others
//! (`open host service`) to the actions of sekisho's gates that guard them (X15).

use crate::Said;
use ritsu_base::naming::Name;
use std::path::Path;

/// A map of sakai's, as the checks across the borders read it (X14).
#[derive(Clone, Debug, PartialEq)]
pub struct MapFacts {
    /// The map file, from the root.
    pub file: String,
    /// The names of its contexts, in the order the map lists them.
    pub contexts: Vec<String>,
    pub relationships: Vec<MapRelationship>,
}

/// One relationship between two contexts, as the `.ctx` of `from` writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapRelationship {
    pub from: String,
    pub to: String,
    /// The words the `.ctx` starts it with: `upstream`, `downstream`, `shared kernel with`,
    /// `partnership with`, `separate ways from`.
    pub words: String,
    /// `separate ways from`: the two have nothing to do with each other.
    pub separate: bool,
    /// Where it is written, from the root.
    pub file: String,
    pub line: usize,
}

/// What sakai answers for a map.
pub trait Maps {
    /// The map at `map` (a `.ctx` from the root), when it is a map and passes the stages of sakai's
    /// check that decide its contexts, their relationships and who owns what (its words, names and
    /// paths, and its owners); None when the file is a context file, not a map; else what those
    /// stages say. The stages after them (the references, the patterns, the mappings) do not change
    /// which context a file is in, and are the map's own check's to say.
    fn map(&self, root: &Path, map: &str) -> Result<Option<MapFacts>, Vec<Said>>;

    /// The context of the map at `map` that the file at `file` (from the root) belongs to, as
    /// sakai's check decides it (the context of the deepest entry of `owns` that holds it; a
    /// `layer`, a shared kernel and a published language must agree with it); None when the map
    /// does not cover the file, or covers it and gives it to no context; what the stages of
    /// [`Maps::map`] say when the map does not pass them.
    fn context_of(&self, root: &Path, map: &str, file: &str) -> Result<Option<String>, Vec<Said>>;

    /// Every operation the contexts of the map at `map` open to the others, in the order of the
    /// contexts and of what their `open host service` lines list; what the stages of [`Maps::map`]
    /// say when the map does not pass them. A channel of an AsyncAPI document and a rule's Connect
    /// service are no operations here: none of a contract of the project that a gate guards. None
    /// by default, until sakai answers it.
    fn published_operations(&self, root: &Path, map: &str) -> Result<Vec<PublishedOperation>, Vec<Said>> {
        let _ = (root, map);
        Ok(Vec::new())
    }
}

/// One operation a context of a map opens to the other contexts (sekisho's X15): each method of a
/// service its `open host service` lists from a `.proto` of its published language, and each
/// operation of an OpenAPI document of its published language that `open host service` lists.
#[derive(Clone, Debug, PartialEq)]
pub struct PublishedOperation {
    /// The context, by its name.
    pub context: String,
    /// The operation, as a reference from the root (ritsu's DESIGN 6.2), as a gate's `guards` names
    /// it: `openapi "payments/api/payments.yaml" operation createCharge` (its `operationId`, else its
    /// method and path), `proto "proto/warehouse/v1/stock.proto" service StockService method Reserve`.
    pub operation: Name,
    /// The document says anyone may call it (`security: []`, or a requirement that asks for
    /// nothing): it needs no action to guard it.
    pub open_to_anyone: bool,
    /// Where `open host service` lists it: the context's file, from the root, and the line.
    pub file: String,
    pub line: usize,
}
