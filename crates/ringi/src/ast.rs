//! A `.ringi` as the parser reads it (DESIGN 3): one of four kinds of file, each a list of blocks,
//! with every name, reference and expression at the place it is written.

use crate::lex::Num;

/// Where something is written: the line and the columns, from 1, in characters; `end` is the column
/// after it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub end: usize,
}

/// A word at its place: a reference to a name, or a word of the language's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub span: Span,
}

/// A name as it is declared: the name, and the ASCII name the generated code and the JSON take
/// (`種別(category)`; DESIGN 3.4). An English name is its own ASCII name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
    pub alias: Option<Word>,
}

impl Name {
    /// The name the generated code and the JSON take.
    pub fn ascii(&self) -> &str {
        self.alias.as_ref().map(|a| a.text.as_str()).unwrap_or(&self.text)
    }

    /// The name as written, with its ASCII name when it has one.
    pub fn shown(&self) -> String {
        match &self.alias {
            Some(a) => format!("{}({})", self.text, a.text),
            None => self.text.clone(),
        }
    }
}

/// The four kinds of `.ringi` (DESIGN 3.1), by the first word of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Request,
    Route,
    Organization,
    Masters,
}

impl FileKind {
    pub fn word(self) -> &'static str {
        match self {
            FileKind::Request => "request",
            FileKind::Route => "route",
            FileKind::Organization => "organization",
            FileKind::Masters => "masters",
        }
    }

    /// What the kind is called in Japanese.
    pub fn ja(self) -> &'static str {
        match self {
            FileKind::Request => "申請の種類",
            FileKind::Route => "共有の経路",
            FileKind::Organization => "組織",
            FileKind::Masters => "マスタ",
        }
    }
}

/// A `.ringi`.
#[derive(Clone, Debug)]
pub struct File {
    pub kind: FileKind,
    pub name: Name,
    pub version: u32,
    pub title: Option<String>,
    pub description: Option<(String, Span)>,
    /// The team that keeps a request (`owner accounting`).
    pub owner: Option<Word>,
    pub effective_from: Option<(i64, Span)>,
    pub uses: Vec<Use>,
    /// A request's form.
    pub form: Vec<Field>,
    /// What a shared route takes (DESIGN 5.11).
    pub inputs: Vec<Field>,
    pub route: Option<Route>,
    pub who: Vec<Who>,
    pub every_stage: Option<Deadlines>,
    pub tables: Vec<NamedTable>,
    pub ticket: Option<Ticket>,
    pub claims: Vec<Claim>,
    pub examples: Option<Examples>,
    pub scenarios: Vec<Scenario>,
    pub labels: Vec<Labels>,
    /// An organization's positions, from the lowest (DESIGN 8.1).
    pub positions: Vec<Titled>,
    /// An organization's kinds of team, from the widest.
    pub team_kinds: Vec<Titled>,
    /// Where an organization's data is (`data "../org"`).
    pub data: Option<(String, Span)>,
    pub masters: Vec<Master>,
    /// The line of each block's first line, by its word (`form`, `every stage`): where a language a
    /// block needs is first needed (E008).
    pub blocks: Vec<(&'static str, usize)>,
}

/// What a `use` reads (DESIGN 3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseKind {
    Organization,
    Masters,
    Calendar,
    Book,
    Flow,
    Rule,
    Dates,
    Route,
    Gate,
}

impl UseKind {
    pub const ALL: [UseKind; 9] = [UseKind::Organization, UseKind::Masters, UseKind::Calendar, UseKind::Book, UseKind::Flow, UseKind::Rule, UseKind::Dates, UseKind::Route, UseKind::Gate];

    pub fn word(self) -> &'static str {
        match self {
            UseKind::Organization => "organization",
            UseKind::Masters => "masters",
            UseKind::Calendar => "calendar",
            UseKind::Book => "book",
            UseKind::Flow => "flow",
            UseKind::Rule => "rule",
            UseKind::Dates => "dates",
            UseKind::Route => "route",
            UseKind::Gate => "gate",
        }
    }

    pub fn of_word(w: &str) -> Option<UseKind> {
        UseKind::ALL.into_iter().find(|k| k.word() == w)
    }

    /// The extension of the file it reads.
    pub fn extension(self) -> &'static str {
        match self {
            UseKind::Organization | UseKind::Masters | UseKind::Route => "ringi",
            UseKind::Calendar | UseKind::Dates => "cal",
            UseKind::Book => "book",
            UseKind::Flow => "flow",
            UseKind::Rule => "rule",
            UseKind::Gate => "gate",
        }
    }

    /// The first word of the file it reads, where one extension has files of two kinds: a `.ringi`
    /// of the kind, and a `.cal` that is a calendar or a dates file (koyomi's).
    pub fn first_word(self) -> Option<&'static str> {
        match self {
            UseKind::Organization => Some("organization"),
            UseKind::Masters => Some("masters"),
            UseKind::Route => Some("route"),
            UseKind::Calendar => Some("calendar"),
            UseKind::Dates => Some("dates"),
            _ => None,
        }
    }

    /// The language that reads the file, when it is not ringi.
    pub fn tool(self) -> Option<&'static str> {
        match self {
            UseKind::Organization | UseKind::Masters | UseKind::Route => None,
            UseKind::Calendar | UseKind::Dates => Some("koyomi"),
            UseKind::Book => Some("chobo"),
            UseKind::Flow => Some("dandori"),
            UseKind::Rule => Some("rulec"),
            UseKind::Gate => Some("sekisho"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Use {
    pub kind: UseKind,
    /// None for a gate, which has no alias (sekisho's `use gate`).
    pub alias: Option<Name>,
    pub path: String,
    pub path_span: Span,
    pub line: usize,
}

/// A field's type (DESIGN 4.1).
#[derive(Clone, Debug, PartialEq)]
pub enum TypeExpr {
    Text,
    LongText,
    /// `number`, `money[JPY]`, `rate[step 0.1%]`, `mass[kg]`: a number in a unit.
    Num(ritsu_units::Unit),
    Date,
    /// One of the values written under the field.
    Choice,
    /// One of a master's rows (`m.accounts`), or one of the values of a rule's enum
    /// (`authority.kind`).
    ChoiceFrom(Vec<Word>),
    Attachment,
    Person,
    Team,
    Bool,
    Rows,
}

impl TypeExpr {
    pub fn word(&self) -> String {
        match self {
            TypeExpr::Text => "text".into(),
            TypeExpr::LongText => "long text".into(),
            TypeExpr::Num(u) => unit_type(u),
            TypeExpr::Date => "date".into(),
            TypeExpr::Choice => "choice".into(),
            TypeExpr::ChoiceFrom(p) => format!("choice from {}", dotted(p)),
            TypeExpr::Attachment => "attachment".into(),
            TypeExpr::Person => "person".into(),
            TypeExpr::Team => "team".into(),
            TypeExpr::Bool => "bool".into(),
            TypeExpr::Rows => "rows".into(),
        }
    }
}

/// A unit's type as rulec writes it: `number`, `money[JPY]`, `rate[step 0.1%]`, `mass[kg]`.
pub fn unit_type(u: &ritsu_units::Unit) -> String {
    use ritsu_units::Dim;
    match &u.dim {
        Dim::Number => "number".into(),
        Dim::Rate => match u.step {
            Some(s) => format!("rate[step {}%]", crate::lex::decimal(s.mul(ritsu_units::Rat::int(100)))),
            None => "rate".into(),
        },
        Dim::Money(_) => match u.tax {
            Some(t) => format!("money[{}, {}]", u.unit, t.word()),
            None => format!("money[{}]", u.unit),
        },
        d => format!("{}[{}]", d.word(), u.unit),
    }
}

/// Words joined with dots: `m.systems.owner`.
pub fn dotted(p: &[Word]) -> String {
    p.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(".")
}

/// A bound of a `range` or a `count`: the value, whether it is included, and where it is written.
#[derive(Clone, Debug)]
pub struct Bound {
    pub value: Expr,
    pub inclusive: bool,
}

/// `range >=1 <=20`, `count <=100`: the lower end, the upper end, and where the mark is written.
#[derive(Clone, Debug)]
pub struct Bounds {
    pub lo: Option<Bound>,
    pub hi: Option<Bound>,
    pub span: Span,
}

/// `round down(1JPY)`.
#[derive(Clone, Debug)]
pub struct Round {
    pub mode: Word,
    pub step: Expr,
    pub span: Span,
}

/// A field of a form, of a row of a `rows` field, of a master, or an input of a shared route.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: Name,
    pub ty: TypeExpr,
    pub ty_span: Span,
    pub title: Option<String>,
    pub range: Option<Bounds>,
    pub count: Option<Bounds>,
    pub optional: Option<Span>,
    pub required_when: Option<Expr>,
    pub shown_when: Option<Expr>,
    pub approvers_only: Option<Span>,
    /// `open in office`: the calendar whose open days the date may be.
    pub open_in: Option<Word>,
    pub round: Option<Round>,
    /// `= <expression>`: the requester does not enter it (DESIGN 4.4).
    pub computed: Option<Expr>,
    /// The values of a `choice`, each with its text.
    pub options: Vec<Titled>,
    /// The titles of the values of a rule's enum a `choice from` chooses from, as the lines under
    /// it write them (`goods "Goods and services"`, DESIGN 4.2): what a screen shows a value as, in
    /// the file's language. Empty for any other field.
    pub value_titles: Vec<Titled>,
    /// The fields of a row of a `rows` field.
    pub children: Vec<Field>,
    pub line: usize,
}

/// A name with its text: a value of a choice, a position, a kind of team.
#[derive(Clone, Debug)]
pub struct Titled {
    pub name: Name,
    pub title: Option<String>,
    pub line: usize,
}

/// An expression or a condition, in rulec's spelling (DESIGN 3.3, 4.5).
#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinOp {
    pub fn sign(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Is,
    IsNot,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub fn sign(self) -> &'static str {
        match self {
            CmpOp::Is => "is",
            CmpOp::IsNot => "is not",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Num(Num),
    Date(i64),
    Str(String),
    None,
    Bool(bool),
    /// A name, or names joined with dots: `amount`, `requester.department`, `authority.board`.
    Path(Vec<Word>),
    /// `sum(items.price)`.
    Sum(Vec<Word>),
    /// `lookup m.vendors.name by vendor`: a master's field in the row a field chose (DESIGN 4.6).
    Lookup(Vec<Word>, Word),
    /// `days business days in office`: a number of business days, counted in a calendar.
    BusinessDays(Box<Expr>, Option<Word>),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    /// The rows of a `rows` field in a scenario's form: `[{what: "Server", price: 1_200_000JPY}]`.
    List(Vec<Expr>),
    Record(Vec<(Word, Expr)>),
    /// `person "e7001"`: the value of a `person` field, where a form's values are written (a
    /// scenario's `form`, a cell of the examples, `simulate --form`, an event's arguments).
    Person(String),
    /// `team legal`: the value of a `team` field, written in the same places.
    Team(Word),
}

impl Expr {
    /// Every expression in it, itself first.
    pub fn walk<'a>(&'a self, out: &mut Vec<&'a Expr>) {
        out.push(self);
        match &self.kind {
            ExprKind::BusinessDays(e, _) | ExprKind::Neg(e) | ExprKind::Not(e) => e.walk(out),
            ExprKind::Bin(_, a, b) | ExprKind::Cmp(_, a, b) | ExprKind::And(a, b) | ExprKind::Or(a, b) => {
                a.walk(out);
                b.walk(out);
            }
            ExprKind::List(xs) => xs.iter().for_each(|x| x.walk(out)),
            ExprKind::Record(fs) => fs.iter().for_each(|(_, x)| x.walk(out)),
            _ => {}
        }
    }

    /// The first name of every path it reads (`amount`, `requester`, `authority`), with the whole
    /// path: what a condition reads.
    pub fn paths(&self) -> Vec<&[Word]> {
        let mut all = Vec::new();
        self.walk(&mut all);
        all.into_iter()
            .filter_map(|e| match &e.kind {
                ExprKind::Path(p) | ExprKind::Sum(p) => Some(p.as_slice()),
                ExprKind::Lookup(_, by) => Some(std::slice::from_ref(by)),
                _ => None,
            })
            .collect()
    }
}

/// A quorum of a stage that waits for more than one approval (DESIGN 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quorum {
    /// `all of legal`.
    All,
    /// `at least 2 of legal`.
    AtLeast(u32),
    /// `at least 60% of board`.
    Percent(u32),
}

/// Who answers a stage, as a stage line or a cell writes it (DESIGN 5.1).
#[derive(Clone, Debug)]
pub enum Spec {
    /// No stage: an empty cell.
    Empty,
    /// One way to choose, or several joined with `and`, one approval from each (`dept_head and
    /// accounting`).
    Who(Vec<Word>),
    /// `at least 60% of board`, `at least 2 of legal`, `all of legal`.
    Quorum(Quorum, Word, Span),
    /// `circulate to general_affairs`: told, and not waited for.
    Circulate(Word),
    /// `note from summarize`: a flow's summary put on the cards after it.
    Note(Word),
    /// `authority.decider`: the way to choose a rule's enum output names.
    RuleOutput(Vec<Word>),
    /// A cell of a route that chooses the route (`-> route`): a shared route's alias.
    Route(Word),
}

impl Spec {
    /// The ways to choose it names.
    pub fn who(&self) -> Vec<&Word> {
        match self {
            Spec::Who(ws) => ws.iter().collect(),
            Spec::Quorum(_, w, _) | Spec::Circulate(w) => vec![w],
            _ => Vec::new(),
        }
    }

    /// Whether a person answers it: a stage that circulates or a summary does not.
    pub fn approves(&self) -> bool {
        matches!(self, Spec::Who(_) | Spec::Quorum(..) | Spec::RuleOutput(_))
    }

    /// How the stage is written.
    pub fn shown(&self) -> String {
        match self {
            Spec::Empty => String::new(),
            Spec::Who(ws) => ws.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" and "),
            Spec::Quorum(Quorum::All, w, _) => format!("all of {}", w.text),
            Spec::Quorum(Quorum::AtLeast(n), w, _) => format!("at least {n} of {}", w.text),
            Spec::Quorum(Quorum::Percent(n), w, _) => format!("at least {n}% of {}", w.text),
            Spec::Circulate(w) => format!("circulate to {}", w.text),
            Spec::Note(w) => format!("note from {}", w.text),
            Spec::RuleOutput(p) => dotted(p),
            Spec::Route(w) => w.text.clone(),
        }
    }
}

/// A test of an input cell of a route's table, in rulec's spelling.
#[derive(Clone, Debug)]
pub enum Test {
    /// `-`: any value.
    Any,
    /// `travel`, `travel, expense`, `not: travel`, `none`.
    Values { values: Vec<Expr>, not: bool },
    /// `>30_000JPY <=100_000JPY`.
    Compare(Vec<(CmpOp, Expr)>),
}

/// A route (DESIGN 5.3, 5.11).
#[derive(Clone, Debug)]
pub enum Route {
    Table(RouteTable),
    Stages(Vec<StageLine>),
    /// `route standard(long: days > 5)`: a shared route, used with what it takes.
    Shared { alias: Word, args: Vec<Arg>, line: usize },
}

/// A route as a table: the inputs, `->`, the stages.
#[derive(Clone, Debug)]
pub struct RouteTable {
    /// The input columns: a field, or a rule's output (`authority.board`).
    pub inputs: Vec<Vec<Word>>,
    pub stages: Vec<Name>,
    /// Whether the outputs choose a whole route (`-> route`).
    pub chooses_route: bool,
    pub rows: Vec<RouteRow>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct RouteRow {
    pub tests: Vec<(Test, Span)>,
    pub stages: Vec<(Spec, Span)>,
    pub line: usize,
}

/// A stage on a line of its own: its name, who answers, when it is there, and the deadlines it
/// writes for itself.
#[derive(Clone, Debug)]
pub struct StageLine {
    pub name: Name,
    pub spec: Spec,
    pub spec_span: Span,
    pub when: Option<Expr>,
    pub deadlines: Option<Deadlines>,
    pub line: usize,
}

/// A way to choose an approver (DESIGN 5.2).
#[derive(Clone, Debug)]
pub struct Who {
    pub name: Name,
    pub how: How,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum How {
    /// `person "e9001"`.
    Person(String, Span),
    /// `team accounting`.
    Team(Word),
    /// `manager of requester`.
    ManagerOf,
    /// `head of requester.department`: the kind of team.
    HeadOf(Word),
    /// `nearest division_head above requester`: the position.
    Nearest(Word),
    /// `chosen by requester in reviewer`, `chosen by previous approver in reviewer`.
    ChosenBy { previous: bool, field: Word },
    /// `lookup m.systems.owner by system`.
    Lookup { path: Vec<Word>, by: Word },
    /// `agent precheck_agent, else manager`.
    Agent { flow: Word, otherwise: Word },
}

impl How {
    /// Whether it names one person at most: a quorum of more than one of it cannot be reached (E304).
    pub fn one_person(&self) -> bool {
        !matches!(self, How::Team(_) | How::Agent { .. })
    }
}

/// When a business day count ends: at the end of the day, or at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    EndOfDay,
    /// Minutes after midnight.
    Time(u32),
}

/// What happens when a stage's deadline passes.
#[derive(Clone, Debug)]
pub enum Late {
    Escalate(Option<Word>),
    Expire,
    Stay,
}

impl Late {
    /// As it is written: `escalate by next_up`, `expire`, `stay`.
    pub fn written(&self) -> String {
        match self {
            Late::Escalate(Some(t)) => format!("escalate by {}", t.text),
            Late::Escalate(None) => "escalate".into(),
            Late::Expire => "expire".into(),
            Late::Stay => "stay".into(),
        }
    }

    /// A `when late` line as it is written: `when late escalate by next_up, then expire`.
    pub fn line(first: &Late, then: Option<&Late>) -> String {
        match then {
            Some(t) => format!("when late {}, then {}", first.written(), t.written()),
            None => format!("when late {}", first.written()),
        }
    }
}

/// What happens when a stage has no approver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenEmpty {
    Stay,
    /// `escalate`, with what happens where nobody is above to go up to (`, then skip`, `, then
    /// stay`, `, then board`); None where nothing is written, which is `then stay`.
    Escalate(Option<AtTop>),
    Skip,
}

/// What a stage that goes up when it is empty does where nobody is above (DESIGN 5.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtTop {
    Stay,
    Skip,
    /// It goes to this way to choose, declared in `who` of the file the line is written in.
    To(Word),
}

impl WhenEmpty {
    /// The line as it is written, `then` and all: `when empty escalate, then skip`. What is not
    /// written is not shown.
    pub fn written(&self) -> String {
        match self {
            WhenEmpty::Stay => "when empty stay".into(),
            WhenEmpty::Skip => "when empty skip".into(),
            WhenEmpty::Escalate(None) => "when empty escalate".into(),
            WhenEmpty::Escalate(Some(AtTop::Stay)) => "when empty escalate, then stay".into(),
            WhenEmpty::Escalate(Some(AtTop::Skip)) => "when empty escalate, then skip".into(),
            WhenEmpty::Escalate(Some(AtTop::To(w))) => format!("when empty escalate, then {}", w.text),
        }
    }

    /// The way to choose it goes to where nobody is above (`then board`), when it says one.
    pub fn at_top_way(&self) -> Option<&Word> {
        match self {
            WhenEmpty::Escalate(Some(AtTop::To(w))) => Some(w),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Deadline {
    Due { days: u32, calendar: Option<Word>, at: At, line: usize },
    Remind { days: u32, calendar: Option<Word>, at: u32, line: usize },
    Late { first: Late, then: Option<Late>, line: usize },
    Empty { what: WhenEmpty, line: usize },
}

/// `every stage`, or the lines under a stage.
#[derive(Clone, Debug)]
pub struct Deadlines {
    pub lines: Vec<Deadline>,
    pub line: usize,
}

/// A table a route and the deadlines read (`table next_up`): a column that names a field holds that
/// field's values, and any other column the names of ways to choose an approver.
#[derive(Clone, Debug)]
pub struct NamedTable {
    pub name: Name,
    pub inputs: Vec<Name>,
    pub outputs: Vec<Name>,
    pub rows: Vec<RouteRow>,
    pub line: usize,
}

/// The ways a ticket ends, or is taken somewhere, that `on` can act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Submitted,
    Approved,
    SentBack,
    Rejected,
    Withdrawn,
    Expired,
}

impl Ending {
    pub fn word(self) -> &'static str {
        match self {
            Ending::Submitted => "submitted",
            Ending::Approved => "approved",
            Ending::SentBack => "sent back",
            Ending::Rejected => "rejected",
            Ending::Withdrawn => "withdrawn",
            Ending::Expired => "expired",
        }
    }
}

/// `name: value` in a call or a shared route's use.
#[derive(Clone, Debug)]
pub struct Arg {
    pub name: Word,
    pub value: Expr,
}

/// `budget.reserve(request: ticket.id, …)`, `pay(request_id: ticket.id, …)`.
#[derive(Clone, Debug)]
pub struct Call {
    pub target: Vec<Word>,
    pub args: Vec<Arg>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Hold,
    Post,
    Void,
    Launch,
}

impl ActionKind {
    pub fn word(self) -> &'static str {
        match self {
            ActionKind::Hold => "hold",
            ActionKind::Post => "post",
            ActionKind::Void => "void",
            ActionKind::Launch => "launch",
        }
    }
}

#[derive(Clone, Debug)]
pub enum TicketLine {
    SelfApproval { allowed: bool, line: usize },
    /// `same approver approves once`: a stage whose one approver approved the stage right before
    /// it is done with that approval. With `, at the last stage` (`last`), a stage whose one
    /// approver alone answers a later stage too waits for the last such stage, and is done with
    /// the approval given there (DESIGN 5.12).
    SameApprover { last: bool, line: usize },
    /// `submitted by everyone except team board`: who may submit the request, and who of them may
    /// not (DESIGN 5.13). `col` is where `submitted` is.
    SubmittedBy { groups: Vec<Group>, except: Vec<Group>, line: usize, col: usize },
    SendBack { to_requester: bool, times: u32, line: usize },
    Resubmit { days: u32, calendar: Option<Word>, line: usize },
    /// `withdraw until decide`; None for `until it is decided`.
    Withdraw { until: Option<Word>, line: usize },
    Retract { line: usize },
    Pause { days: u32, calendar: Option<Word>, line: usize },
    ApproversEdit { fields: Vec<Word>, line: usize },
    /// `comment required for reject, send back`: the operations a person gives with a reason, their
    /// comment, which may then be neither empty nor white space alone (DESIGN 7.2). Each operation
    /// once, in the order written.
    CommentRequired { ops: Vec<(Reasoned, Span)>, line: usize },
    On { endings: Vec<(Ending, Span)>, action: ActionKind, call: Call, when: Option<Expr>, line: usize },
}

/// An operation `comment required for` names (DESIGN 7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reasoned {
    Reject,
    SendBack,
}

impl Reasoned {
    /// As the line writes it: `reject`, `send back`.
    pub fn word(self) -> &'static str {
        match self {
            Reasoned::Reject => crate::kw::REASONED[0],
            Reasoned::SendBack => crate::kw::REASONED[1],
        }
    }

    /// Its event's name in the contract: `reject`, `send_back`.
    pub fn event(self) -> &'static str {
        match self {
            Reasoned::Reject => "reject",
            Reasoned::SendBack => "send_back",
        }
    }
}

impl Ticket {
    /// The operations its `comment required for` line names, in the order written; none without
    /// the line.
    pub fn reasoned(&self) -> Vec<Reasoned> {
        self.lines.iter().filter_map(|l| if let TicketLine::CommentRequired { ops, .. } = l { Some(ops.iter().map(|(o, _)| *o)) } else { None }).flatten().collect()
    }
}

impl TicketLine {
    /// The line it is written on.
    pub fn line(&self) -> usize {
        match self {
            TicketLine::SelfApproval { line, .. }
            | TicketLine::SameApprover { line, .. }
            | TicketLine::SubmittedBy { line, .. }
            | TicketLine::SendBack { line, .. }
            | TicketLine::Resubmit { line, .. }
            | TicketLine::Withdraw { line, .. }
            | TicketLine::Retract { line }
            | TicketLine::Pause { line, .. }
            | TicketLine::ApproversEdit { line, .. }
            | TicketLine::CommentRequired { line, .. }
            | TicketLine::On { line, .. } => *line,
        }
    }
}

/// The people a `submitted by` line names (DESIGN 5.13), in a version of the organization.
#[derive(Clone, Debug)]
pub enum Group {
    /// `everyone`: every person; `everyone in sales_div`: the people whose primary team is the team
    /// or one below it.
    Everyone { within: Option<Word>, span: Span },
    /// `team board`: the members of the team, as `who`'s `team` names them.
    Team(Word),
    /// `position manager`, `position manager or higher`, either `in sales_div`: by the position in
    /// the primary team, the positions compared as `nearest … above` compares them.
    Position { position: Word, higher: bool, within: Option<Word> },
    /// `person "e9001"`.
    Person(String, Span),
}

impl Group {
    /// The group as it is written, spaced as the language spaces it.
    pub fn shown(&self) -> String {
        let within = |w: &Option<Word>| w.as_ref().map(|t| format!(" in {}", t.text)).unwrap_or_default();
        match self {
            Group::Everyone { within: w, .. } => format!("everyone{}", within(w)),
            Group::Team(t) => format!("team {}", t.text),
            Group::Position { position, higher, within: w } => format!("position {}{}{}", position.text, if *higher { " or higher" } else { "" }, within(w)),
            Group::Person(id, _) => format!("person \"{id}\""),
        }
    }

    /// The groups of a `submitted by` line as it is written: `everyone except team board`.
    pub fn line_shown(groups: &[Group], except: &[Group]) -> String {
        let list = |gs: &[Group]| gs.iter().map(Group::shown).collect::<Vec<_>>().join(", ");
        if except.is_empty() { list(groups) } else { format!("{} except {}", list(groups), list(except)) }
    }
}

#[derive(Clone, Debug)]
pub struct Ticket {
    pub lines: Vec<TicketLine>,
    pub line: usize,
}

/// A claim about a request (DESIGN 6.4).
#[derive(Clone, Debug)]
pub struct Claim {
    pub name: Word,
    pub body: ClaimBody,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum ClaimBody {
    /// `decide is travel_agent only when amount <= 30_000JPY`.
    OnlyWhen { stage: Word, spec: Spec, when: Expr },
    /// `possible second is dept_head and accounting`.
    Possible { stage: Word, spec: Spec },
    /// `every stage reminds before it is due`.
    RemindsBeforeDue,
    /// `every ticket ends within 93 business days`.
    EndsWithin { days: u32 },
}

/// `examples`: rows of a route's inputs and the stages they come to.
#[derive(Clone, Debug)]
pub struct Examples {
    pub inputs: Vec<Vec<Word>>,
    pub stages: Vec<Word>,
    pub rows: Vec<ExampleRow>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct ExampleRow {
    pub values: Vec<(Expr, Span)>,
    pub stages: Vec<(Spec, Span)>,
    pub line: usize,
}

/// A scenario (DESIGN 10.2): who asks, what the books hold at the start, the form, and the events
/// with the stage and the state each comes to.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: Word,
    pub requester: Option<Word>,
    pub given: Vec<Call>,
    pub form: Vec<(Word, Expr)>,
    pub events: Vec<Event>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct Event {
    /// The day and the minute.
    pub at: Option<(i64, u32)>,
    pub event: Word,
    /// What the event carries, after its name in brackets (DESIGN 10.2): `send back(to: first)`,
    /// `edit(reviewer: person "e9002")`, `agent answered(verdict: ok)`.
    pub args: Vec<Arg>,
    pub by: Option<Word>,
    /// What the person writes with the event, in the `comment` column (DESIGN 10.2).
    pub comment: Option<Word>,
    pub stage: Option<Word>,
    pub status: Word,
    pub line: usize,
}

/// `labels ja`: the texts of the request in another language (DESIGN 4.8).
#[derive(Clone, Debug)]
pub struct Labels {
    pub lang: Word,
    pub entries: Vec<Label>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub enum LabelKey {
    /// The request's own title.
    Request,
    /// `category`, `category.travel`, `items.price`.
    Field(Vec<Word>),
    Stage(Word),
    Who(Word),
}

#[derive(Clone, Debug)]
pub struct Label {
    pub key: LabelKey,
    pub text: String,
    pub span: Span,
    pub line: usize,
}

/// A master of a `masters` file (DESIGN 4.6).
#[derive(Clone, Debug)]
pub struct Master {
    pub name: Name,
    pub title: Option<String>,
    pub key: Option<Word>,
    pub fields: Vec<Field>,
    pub data: Option<(String, Span)>,
    pub line: usize,
}
