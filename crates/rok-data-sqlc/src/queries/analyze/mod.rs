//! Schema-aware analysis of annotated queries: parameter types, result
//! columns, and the Rust shape of the result.
//!
//! Types are inferred only from contexts that determine them unambiguously:
//! a comparison, `IN`, `= ANY`, `BETWEEN`, or `LIKE` against a known column;
//! arithmetic with a numeric column; an `INSERT` value (of `VALUES` or a
//! `SELECT`) or `UPDATE … SET` target; `LIMIT`/`OFFSET`; and explicit casts. Anything else must be
//! declared with `-- param:` or `-- column:`; the generator never guesses. A
//! non-recursive CTE is analyzed like a `SELECT` (one that modifies data like
//! its `INSERT`, `UPDATE` or `DELETE`, by its `RETURNING`) and then acts as a
//! table for the statements after it; the final statement gives the result.
//! `UPDATE … FROM` and `DELETE … USING` see their target and the other
//! tables together, like a join. A subquery sees its own tables first, then
//! the enclosing query's. `UNION`, `INTERSECT` and `EXCEPT` type each column
//! from the type their branches share, as `CASE` does.
//! `generate_series`, `unnest` and `jsonb_to_recordset` in `FROM` act as
//! tables typed from their signatures (`set_returning.rs`).

mod conflict;
mod params;
mod resolve;
mod results;
mod returns;
mod set_returning;
mod statements;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Deref;
use std::rc::Rc;

use heck::ToSnakeCase;
use sqlparser::ast::Query;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use self::conflict::take_conflict_predicate;
use self::results::result_shape;
use super::annotations::{QueryDef, QueryFailure, ReturnMode};
use crate::parser::TableFailure;
use crate::parser::naming;
use crate::schema::{EnumType, SqlFunction, Table};

/// A bound parameter, in placeholder order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The placeholder number (`$1` is 1).
    pub index: usize,
    pub name: String,
    pub rust_type: String,
}

/// One column of a query's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputColumn {
    /// The name PostgreSQL reports for the column.
    pub sql_name: String,
    pub rust_name: String,
    pub rust_type: String,
}

/// The Rust type a query's rows decode into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultShape {
    /// `:exec` / `:execrows`: rows are not read.
    None,
    /// Every column of a generated resource, in order: its row struct is reused.
    Model { module: String, struct_name: String },
    /// A single column, decoded with `query_scalar`.
    Scalar(String),
    /// Several columns: a dedicated `{Name}Row` struct.
    Row(Vec<OutputColumn>),
    /// `-- returns: <Other>Row`: another query's row struct, its columns
    /// the same (`columns`); no struct of its own.
    Shared {
        name: String,
        columns: Vec<OutputColumn>,
    },
    /// `-- returns: path::Type`: a hand-written `sqlx::FromRow` type the
    /// rows decode into.
    External(String),
}

/// A query ready for code generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzedQuery {
    pub name: String,
    pub fn_name: String,
    pub mode: ReturnMode,
    /// The SQL as written, without its trailing `;`.
    pub sql: String,
    pub params: Vec<Param>,
    pub result: ResultShape,
    /// `-- rok:params`: the shape the query asks for, if it does.
    pub params_shape: Option<crate::ParamsShape>,
    /// The project's `query_params`, which applies when the query doesn't
    /// say; `rok gen` sets it.
    pub params_default: crate::ParamsShape,
}

/// Analyzes each query against the schema.
///
/// `tables` are all generatable tables, `enums` the generated enums (a cast
/// to a database enum type infers it), `functions` the schema's functions (a
/// call to one takes its `RETURNS` type), `generated_modules` the resource
/// modules emitted in this run (whose row structs may be reused), and
/// `unsupported` the tables the schema parser rejected, used to explain why a
/// query referencing one of them cannot be generated.
pub fn analyze_queries(
    defs: &[QueryDef],
    tables: &[Table],
    enums: &[EnumType],
    functions: &[SqlFunction],
    generated_modules: &HashSet<String>,
    unsupported: &[TableFailure],
) -> (Vec<AnalyzedQuery>, Vec<QueryFailure>) {
    let mut analyzed = Vec::new();
    let mut failures = Vec::new();
    let mut fn_names = HashSet::new();
    // A `-- rok:skip` query is left out on purpose: neither generated nor
    // a failure.
    for def in defs.iter().filter(|def| def.skip.is_none()) {
        let context = Context {
            tables,
            enums,
            functions,
            unsupported,
            generated_modules,
        };
        match analyze_one(def, &context) {
            Ok(query) if !fn_names.insert(query.fn_name.clone()) => failures.push(
                QueryFailure::new(
                    &def.name,
                    format!(
                        "function name `{}` is already used by another query",
                        query.fn_name
                    ),
                )
                .at_line(def.line),
            ),
            Ok(query) => analyzed.push(query),
            Err(reason) => failures.push(QueryFailure::new(&def.name, reason).at_line(def.line)),
        }
    }
    // `-- returns:`, once every row type of the file is known.
    let rows: HashMap<String, Vec<OutputColumn>> = analyzed
        .iter()
        .filter_map(|query| match &query.result {
            ResultShape::Row(columns) => Some((format!("{}Row", query.name), columns.clone())),
            _ => None,
        })
        .collect();
    analyzed.retain_mut(|query| {
        let Some(def) = defs.iter().find(|def| def.name == query.name) else {
            return true;
        };
        let Some(target) = &def.returns else {
            return true;
        };
        match returns::resolve(
            target,
            &query.name,
            &query.result,
            &rows,
            tables,
            generated_modules,
        ) {
            Ok(shape) => {
                query.result = shape;
                true
            }
            Err(reason) => {
                failures.push(QueryFailure::new(&def.name, reason).at_line(def.line));
                false
            }
        }
    });
    (analyzed, failures)
}

struct Context<'a> {
    tables: &'a [Table],
    /// The generated enums, for casts to an enum type.
    enums: &'a [EnumType],
    /// The schema's functions, for calls to them.
    functions: &'a [SqlFunction],
    unsupported: &'a [TableFailure],
    generated_modules: &'a HashSet<String>,
}

fn analyze_one(def: &QueryDef, context: &Context<'_>) -> Result<AnalyzedQuery, String> {
    let fn_name = naming::rust_identifier(&def.name.to_snake_case())?;
    let conflict = take_conflict_predicate(&def.sql)?;
    let statements = Parser::parse_sql(&PostgreSqlDialect {}, &conflict.sql)
        .map_err(|error| format!("invalid SQL: {error}"))?;
    let statement = match statements.as_slice() {
        [statement] => statement,
        [] => return Err("the query has no SQL statement".to_owned()),
        many => {
            return Err(format!(
                "the query contains {} statements; generate one function per statement and run them in a caller-owned transaction (`&mut *tx`)",
                many.len()
            ));
        }
    };

    if def.mode == ReturnMode::Page {
        super::page::check(statement)?;
    }

    let mut analysis = Analysis::new(context);
    analysis.conflict_predicate = conflict.predicate;
    let outputs = analysis.statement(statement)?;
    let params = analysis.params(statement, def)?;
    let result = result_shape(def, outputs, context)?;

    Ok(AnalyzedQuery {
        name: def.name.clone(),
        fn_name,
        mode: def.mode,
        sql: def
            .sql
            .trim_end()
            .trim_end_matches(';')
            .trim_end()
            .to_owned(),
        params,
        result,
        params_shape: def.params_shape,
        params_default: crate::ParamsShape::default(),
    })
}

/// A table visible to column references, under its alias or name.
struct ScopeEntry<'a> {
    qualifier: String,
    table: TableRef<'a>,
    /// An outer join can produce rows without a match on this side.
    outer_nullable: bool,
}

/// A result column with where it came from, for model reuse.
struct Output {
    sql_name: String,
    rust_type: Option<String>,
    /// `(resource module, column index)` for a plain column reference.
    origin: Option<(String, usize)>,
    nullable_origin: bool,
    /// Why the type could not be inferred, reported unless overridden.
    unknown_reason: Option<String>,
    /// Why the column has no name, reported unless a CTE column list names
    /// it; `sql_name` is empty then.
    name_error: Option<String>,
}

/// What a `FROM` item names: a schema table, or a CTE's or set-returning
/// function's result columns shaped as a table.
#[derive(Clone)]
enum TableRef<'a> {
    Schema(&'a Table),
    Cte(Rc<Table>),
}

impl TableRef<'_> {
    /// The resource module whose row struct could be reused for this
    /// table's columns; a CTE has none.
    fn module(&self) -> Option<&str> {
        match self {
            TableRef::Schema(table) => Some(&table.module_name),
            TableRef::Cte(_) => None,
        }
    }
}

impl Deref for TableRef<'_> {
    type Target = Table;

    fn deref(&self) -> &Table {
        match self {
            TableRef::Schema(table) => table,
            TableRef::Cte(table) => table,
        }
    }
}

struct Slot {
    name: String,
    /// The parameter's type: a typed id when every column naming it has the
    /// same one, otherwise the plain type.
    base: String,
    /// The plain type under a typed id (`i64` for `CategoryId`); `base` itself
    /// for other types.
    raw: String,
    nullable: bool,
}

struct Analysis<'a, 'c> {
    context: &'c Context<'a>,
    scope: Vec<ScopeEntry<'a>>,
    /// Where each enclosing subquery's own tables start in `scope`,
    /// innermost last: a subquery sees the tables of the queries around it,
    /// and its own come first.
    levels: Vec<usize>,
    /// CTEs visible by name, in definition order.
    ctes: Vec<Rc<Table>>,
    /// How many queries enclose the one being analyzed: 1 for the
    /// statement's own, more inside a CTE or subquery.
    query_depth: usize,
    slots: BTreeMap<usize, Slot>,
    /// The type of each analyzed subquery's one column, by its address in
    /// the statement (which outlives the analysis); `None` when it returns
    /// several columns or the engine can't type it.
    subquery_types: HashMap<*const Query, Option<String>>,
    /// The analyzed subqueries that are never NULL: their one column is not
    /// optional and they always return one row.
    never_null_subqueries: HashSet<*const Query>,
    /// The `WHERE` of an `ON CONFLICT (cols) WHERE … DO` target, taken out of
    /// the SQL before parsing (see [`take_conflict_predicate`]).
    conflict_predicate: Option<sqlparser::ast::Expr>,
}

impl<'a, 'c> Analysis<'a, 'c> {
    fn new(context: &'c Context<'a>) -> Self {
        Self {
            context,
            scope: Vec::new(),
            levels: Vec::new(),
            ctes: Vec::new(),
            query_depth: 0,
            slots: BTreeMap::new(),
            subquery_types: HashMap::new(),
            never_null_subqueries: HashSet::new(),
            conflict_predicate: None,
        }
    }

    /// Where the innermost query's own tables start in `scope`.
    fn floor(&self) -> usize {
        self.levels.last().copied().unwrap_or(0)
    }

    /// Runs `analyze` in a scope of its own, which sees none of the tables
    /// in scope now (a CTE, a recursive term, an `INSERT … SELECT`), and
    /// restores the scope after.
    fn in_own_scope<T>(&mut self, analyze: impl FnOnce(&mut Self) -> T) -> T {
        let scope = std::mem::take(&mut self.scope);
        let levels = std::mem::take(&mut self.levels);
        let result = analyze(self);
        self.scope = scope;
        self.levels = levels;
        result
    }
}
