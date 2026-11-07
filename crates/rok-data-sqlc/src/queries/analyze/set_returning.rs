//! Set-returning functions in `FROM`: `generate_series`, `unnest`, and
//! `jsonb_to_recordset`/`json_to_recordset` (and their one-row `_record`
//! forms), each typed from its known signature and seen by the rest of the
//! query as a table under its alias.
//!
//! - `generate_series(start, stop[, step])` yields one never-NULL column.
//!   Integer bounds give the wider of `integer` and `bigint`, `numeric`
//!   stays `numeric`; a timestamp series is `timestamp` or `timestamp with
//!   time zone`, and `date` bounds resolve, as in PostgreSQL, to `timestamp
//!   with time zone` (cast the column `::date` for a `NaiveDate`). An
//!   untyped placeholder bound takes the series' type, and the step is the
//!   series' type for numbers and an `interval` for timestamps.
//! - `unnest(a[, b, …])` yields one column per array, of its element type: a
//!   cast placeholder (`$1::uuid[]`) is a `Vec<Uuid>` named after its column
//!   (`{column}_list`). One array's column is never NULL; several arrays'
//!   columns are nullable, since PostgreSQL pads the shorter ones with NULL.
//! - `jsonb_to_recordset(json) AS x(col type, …)` takes its columns from the
//!   column definition list, nullable since a missing key is NULL, and its
//!   argument, when a placeholder, is a `serde_json::Value` named after the
//!   alias.
//!
//! The column names follow PostgreSQL: the alias's column list, else the
//! alias itself for a one-column function, else the function's name.
//! `WITH ORDINALITY` adds a never-NULL `bigint` column, `ordinality` unless
//! the column list names it.

use sqlparser::ast::{
    Expr, FunctionArg, FunctionArgExpr, ObjectName, TableAlias, TableFactor, Value,
};

use super::Analysis;
use super::params::placeholder;
use crate::parser::naming;
use crate::parser::postgres::{ident_name, map_type, object_leaf_name};
use crate::schema::{Column, Shape, Table, rust_type};

/// The functions the engine types in `FROM`, for error messages.
const SUPPORTED: &str = "`generate_series`, `unnest`, `jsonb_to_recordset` and `json_to_recordset`";

const TIMESTAMP: &str = "chrono::NaiveDateTime";
const TIMESTAMP_TZ: &str = "chrono::DateTime<chrono::Utc>";

/// A function's result column before naming: its Rust value type and
/// whether it can be NULL.
struct Typed {
    value_type: String,
    nullable: bool,
}

impl<'a, 'c> Analysis<'a, 'c> {
    /// The table a set-returning function in `FROM` makes, named by its
    /// alias or the function. `None` when `factor` is not a function call.
    pub(super) fn set_returning_table(
        &mut self,
        factor: &TableFactor,
    ) -> Option<Result<Table, String>> {
        Some(match factor {
            TableFactor::Table {
                name,
                alias,
                args: Some(args),
                with_ordinality,
                ..
            } => self.function_table(name, &args.args, alias.as_ref(), *with_ordinality),
            TableFactor::Function {
                name, args, alias, ..
            } => self.function_table(name, args, alias.as_ref(), false),
            TableFactor::UNNEST {
                alias,
                array_exprs,
                with_offset,
                with_ordinality,
                ..
            } => {
                if *with_offset {
                    return Some(Err(
                        "`unnest … WITH OFFSET` is not PostgreSQL; use `WITH ORDINALITY`"
                            .to_owned(),
                    ));
                }
                let arrays: Vec<&Expr> = array_exprs.iter().collect();
                self.unnest_table(&arrays, alias.as_ref(), *with_ordinality)
            }
            _ => return None,
        })
    }

    fn function_table(
        &mut self,
        name: &ObjectName,
        args: &[FunctionArg],
        alias: Option<&TableAlias>,
        with_ordinality: bool,
    ) -> Result<Table, String> {
        let function = object_leaf_name(name).to_ascii_lowercase();
        let arguments = args
            .iter()
            .map(|argument| match argument {
                FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) => Ok(argument),
                other => Err(format!(
                    "argument `{other}` of `{function}` in FROM is not supported; pass it by position"
                )),
            })
            .collect::<Result<Vec<&Expr>, String>>()?;
        match function.as_str() {
            "generate_series" => {
                let names = column_names(&function, alias, 1, with_ordinality)?;
                let column = self.generate_series(&arguments, &names[0])?;
                derived_table(&function, alias, names, vec![column], with_ordinality)
            }
            "unnest" => self.unnest_table(&arguments, alias, with_ordinality),
            "jsonb_to_recordset" | "json_to_recordset" | "jsonb_to_record" | "json_to_record" => {
                self.recordset_table(&function, &arguments, alias, with_ordinality)
            }
            _ => Err(format!(
                "function `{function}` in FROM is not supported: the engine types {SUPPORTED} there"
            )),
        }
    }

    fn generate_series(&mut self, arguments: &[&Expr], column: &str) -> Result<Typed, String> {
        let (start, stop, step) = match arguments {
            [start, stop] => (*start, *stop, None),
            [start, stop, step] => (*start, *stop, Some(*step)),
            _ => {
                return Err(format!(
                    "`generate_series` takes a start, a stop and an optional step; got {} arguments",
                    arguments.len()
                ));
            }
        };
        for (argument, name) in [(Some(start), "start"), (Some(stop), "stop"), (step, "step")] {
            if let Some(argument) = argument {
                self.name_cast_placeholder(argument, name)?;
                self.expr(argument)?;
            }
        }
        let start_type = self.bound_type(start)?;
        let stop_type = self.bound_type(stop)?;
        let series = match (start_type.as_deref(), stop_type.as_deref()) {
            (Some(start), Some(stop)) => Series::of(start, column)?.with(Series::of(stop, column)?),
            (Some(known), None) | (None, Some(known)) => Some(Series::of(known, column)?),
            (None, None) => {
                return Err(format!(
                    "cannot infer the type of `generate_series` for `{column}`; cast a bound (`$1::date`, `$1::int`)"
                ));
            }
        };
        let Some(series) = series else {
            return Err(format!(
                "`generate_series` for `{column}` mixes numbers and timestamps (`{}` and `{}`)",
                start_type.unwrap_or_default(),
                stop_type.unwrap_or_default()
            ));
        };
        if series == Series::SmallInt {
            // `smallint` alone fits the `integer`, `bigint` and `numeric`
            // overloads equally: PostgreSQL rejects the call.
            return Err(format!(
                "`generate_series` over `smallint` alone is ambiguous in PostgreSQL for `{column}`; cast a bound to `int`"
            ));
        }
        let value_type = series.rust_type();
        for (bound, name) in [(start, "start"), (stop, "stop")] {
            self.type_placeholder(bound, name, value_type)?;
        }
        match (step, series.is_timestamp()) {
            (Some(step), true) => self.type_placeholder(step, "step", rust_type::INTERVAL)?,
            (Some(step), false) => self.type_placeholder(step, "step", value_type)?,
            (None, true) => {
                return Err(format!(
                    "`generate_series` over timestamps needs a step for `{column}` (`'1 day'`)"
                ));
            }
            (None, false) => {}
        }
        Ok(Typed {
            value_type: value_type.to_owned(),
            nullable: false,
        })
    }

    /// Names placeholder `expr` when it is cast (`$1::date`), before the cast
    /// itself would name it `param_N`.
    fn name_cast_placeholder(&mut self, expr: &Expr, name: &str) -> Result<(), String> {
        let Expr::Cast {
            expr: inner,
            data_type,
            ..
        } = expr
        else {
            return Ok(());
        };
        let Some(index) = placeholder(inner) else {
            return Ok(());
        };
        let (_, sqlx_type) = map_type(data_type, self.context.enums)?;
        self.assign(index, name, &sqlx_type, &sqlx_type, false)
    }

    /// Types `expr` as `rust_type` when it is a bare placeholder.
    fn type_placeholder(&mut self, expr: &Expr, name: &str, rust_type: &str) -> Result<(), String> {
        match placeholder(expr) {
            Some(index) => self.assign(index, name, rust_type, rust_type, false),
            None => Ok(()),
        }
    }

    /// The type of a `generate_series` bound, or `None` for one PostgreSQL
    /// resolves from the other bound: a bare placeholder or a string
    /// literal.
    fn bound_type(&self, bound: &Expr) -> Result<Option<String>, String> {
        if placeholder(bound).is_some() || is_string_literal(bound) {
            return Ok(None);
        }
        Ok(self
            .value_type(bound)?
            .map(|value_type| self.plain_type(&value_type).to_owned()))
    }

    fn unnest_table(
        &mut self,
        arrays: &[&Expr],
        alias: Option<&TableAlias>,
        with_ordinality: bool,
    ) -> Result<Table, String> {
        if arrays.is_empty() {
            return Err("`unnest` needs an array".to_owned());
        }
        let names = column_names("unnest", alias, arrays.len(), with_ordinality)?;
        let mut columns = Vec::with_capacity(arrays.len());
        for (array, name) in arrays.iter().zip(&names) {
            self.name_cast_placeholder(array, &format!("{}_list", rust_name(name)?))?;
            self.expr(array)?;
            let array_type = self.value_type(array)?;
            let element = match array_type.as_deref().map(Shape::of) {
                Some(Shape::List(element)) if array_type.as_deref() != Some(rust_type::BYTES) => {
                    element.to_owned()
                }
                _ => {
                    return Err(format!(
                        "cannot infer the element type of `unnest({array})`; cast it to an array (`$1::uuid[]`)"
                    ));
                }
            };
            columns.push(Typed {
                value_type: element,
                nullable: arrays.len() > 1,
            });
        }
        derived_table("unnest", alias, names, columns, with_ordinality)
    }

    fn recordset_table(
        &mut self,
        function: &str,
        arguments: &[&Expr],
        alias: Option<&TableAlias>,
        with_ordinality: bool,
    ) -> Result<Table, String> {
        let needs_list = || {
            format!(
                "`{function}` needs a column definition list: `{function}(…) AS x(name type, …)`"
            )
        };
        let alias = alias
            .filter(|alias| !alias.columns.is_empty())
            .ok_or_else(needs_list)?;
        if with_ordinality {
            return Err(format!(
                "`{function}(…) WITH ORDINALITY` is not supported; number the rows with `row_number() OVER ()`"
            ));
        }
        let [argument] = arguments else {
            return Err(format!("`{function}` takes one JSON argument"));
        };
        let json_name = rust_name(&ident_name(&alias.name))?;
        self.name_cast_placeholder(argument, &json_name)?;
        self.type_placeholder(argument, &json_name, "serde_json::Value")?;
        self.expr(argument)?;
        let mut names = Vec::with_capacity(alias.columns.len());
        let mut columns = Vec::with_capacity(alias.columns.len());
        for definition in &alias.columns {
            let name = ident_name(&definition.name);
            let Some(data_type) = &definition.data_type else {
                return Err(format!(
                    "column `{name}` of `{function}` needs a type: {}",
                    needs_list()
                ));
            };
            let (_, value_type) = map_type(data_type, self.context.enums)?;
            names.push(name);
            columns.push(Typed {
                value_type,
                nullable: true,
            });
        }
        derived_table(function, Some(alias), names, columns, false)
    }
}

/// The names of a function's columns: the alias's column list, else the
/// alias for a one-column function, else the function's name; then
/// `ordinality`.
fn column_names(
    function: &str,
    alias: Option<&TableAlias>,
    count: usize,
    with_ordinality: bool,
) -> Result<Vec<String>, String> {
    let listed = alias.map_or(&[][..], |alias| alias.columns.as_slice());
    let total = count + usize::from(with_ordinality);
    if listed.len() > total {
        return Err(format!(
            "`{function}` returns {total} columns but its alias names {}",
            listed.len()
        ));
    }
    if let Some(typed) = listed.iter().find(|column| column.data_type.is_some()) {
        return Err(format!(
            "`{function}` takes column names, not a column definition list (`{}`)",
            ident_name(&typed.name)
        ));
    }
    let default = match alias {
        Some(alias) if count == 1 => ident_name(&alias.name),
        _ => function.to_owned(),
    };
    let names: Vec<String> = (0..total)
        .map(|index| match listed.get(index) {
            Some(column) => ident_name(&column.name),
            None if index == count => "ordinality".to_owned(),
            None => default.clone(),
        })
        .collect();
    if count > 1 && listed.len() < count {
        return Err(format!(
            "`{function}` of {count} arrays needs its columns named: `{function}(…) AS t(a, b)`"
        ));
    }
    Ok(names)
}

/// The table a function's typed columns make, under its alias or name.
fn derived_table(
    function: &str,
    alias: Option<&TableAlias>,
    names: Vec<String>,
    mut columns: Vec<Typed>,
    with_ordinality: bool,
) -> Result<Table, String> {
    if with_ordinality {
        columns.push(Typed {
            value_type: "i64".to_owned(),
            nullable: false,
        });
    }
    let table_name = alias.map_or_else(|| function.to_owned(), |alias| ident_name(&alias.name));
    let mut built: Vec<Column> = Vec::with_capacity(columns.len());
    for (sql_name, typed) in names.into_iter().zip(columns) {
        if built.iter().any(|column| column.sql_name == sql_name) {
            return Err(format!(
                "`{table_name}` names column `{sql_name}` more than once"
            ));
        }
        let rust_type = if typed.nullable {
            format!("Option<{}>", typed.value_type)
        } else {
            typed.value_type.clone()
        };
        built.push(Column {
            rust_name: rust_name(&sql_name)?,
            sql_name,
            // The function's columns exist only within the query.
            sql_type: String::new(),
            rust_type,
            sqlx_type: typed.value_type,
            nullable: typed.nullable,
            is_primary_key: false,
            has_default: false,
            default: None,
            generated: None,
            identity: None,
            is_generated: false,
            value_list: None,
            declared_length: None,
            value_range: None,
            length_range: None,
            typed_id: None,
            is_managed: false,
            id_generation: None,
        });
    }
    Ok(Table {
        schema: None,
        module_name: String::new(),
        struct_name: String::new(),
        sql_name: table_name,
        columns: built,
        primary_key: Vec::new(),
        foreign_keys: Vec::new(),
        unique_keys: Vec::new(),
        key_constraints: Vec::new(),
        named_constraints: Vec::new(),
        version_column: None,
        soft_delete_column: None,
        tenant_column: None,
        relationships: Vec::new(),
        scopes: Vec::new(),
        before_insert_triggers: Vec::new(),
    })
}

fn rust_name(sql_name: &str) -> Result<String, String> {
    naming::rust_identifier(sql_name)
}

fn is_string_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Value(value) => matches!(
            value.value,
            Value::SingleQuotedString(_)
                | Value::DollarQuotedString(_)
                | Value::EscapedStringLiteral(_)
                | Value::UnicodeStringLiteral(_)
        ),
        Expr::Nested(inner) => is_string_literal(inner),
        _ => false,
    }
}

/// The kind of a `generate_series`, from its bounds' types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Series {
    /// Only as a bound: two `smallint` bounds match no overload best.
    SmallInt,
    Integer,
    BigInt,
    Numeric,
    /// Only as a bound: a `date` series resolves to a timestamp.
    Date,
    Timestamp,
    TimestampTz,
}

impl Series {
    fn of(bound_type: &str, column: &str) -> Result<Series, String> {
        Ok(match bound_type {
            "i16" => Series::SmallInt,
            "i32" => Series::Integer,
            "i64" => Series::BigInt,
            "rust_decimal::Decimal" => Series::Numeric,
            "chrono::NaiveDate" => Series::Date,
            TIMESTAMP => Series::Timestamp,
            TIMESTAMP_TZ => Series::TimestampTz,
            other => {
                return Err(format!(
                    "`generate_series` over `{other}` is not supported for `{column}`; its bounds must be integers, numeric, dates or timestamps"
                ));
            }
        })
    }

    fn is_timestamp(self) -> bool {
        self >= Series::Date
    }

    /// The series both bounds resolve to, as PostgreSQL picks the overload;
    /// `None` when one is a number and the other a timestamp.
    fn with(self, other: Series) -> Option<Series> {
        (self.is_timestamp() == other.is_timestamp()).then(|| match self.max(other) {
            // `date` with `date`: both timestamp overloads take an implicit
            // cast, and `timestamp with time zone` is the preferred type.
            Series::Date => Series::TimestampTz,
            widest => widest,
        })
    }

    fn rust_type(self) -> &'static str {
        match self {
            Series::SmallInt | Series::Integer => "i32",
            Series::BigInt => "i64",
            Series::Numeric => "rust_decimal::Decimal",
            Series::Date | Series::TimestampTz => TIMESTAMP_TZ,
            Series::Timestamp => TIMESTAMP,
        }
    }
}
