//! Result columns and the Rust shape of a query's result.

use std::collections::HashSet;

use sqlparser::ast::{
    BinaryOperator, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Ident,
    ObjectNamePart, SelectItem, UnaryOperator, Value,
};

use super::{Analysis, Context, Output, OutputColumn, ResultShape};
use crate::parser::naming;
use crate::parser::postgres::{ident_name, map_type, object_leaf_name};
use crate::queries::annotations::{QueryDef, ReturnMode};
use crate::schema::Column;

impl<'a, 'c> Analysis<'a, 'c> {
    pub(super) fn projection(&mut self, items: &[SelectItem]) -> Result<Vec<Output>, String> {
        let mut outputs = Vec::new();
        for item in items {
            match item {
                SelectItem::Wildcard(_) => {
                    for entry_index in self.floor()..self.scope.len() {
                        if self.scope[entry_index].qualifier == "excluded" {
                            continue;
                        }
                        self.push_table_outputs(entry_index, &mut outputs);
                    }
                }
                SelectItem::QualifiedWildcard(kind, _) => {
                    let qualifier = match kind {
                        sqlparser::ast::SelectItemQualifiedWildcardKind::ObjectName(name) => {
                            object_leaf_name(name)
                        }
                        sqlparser::ast::SelectItemQualifiedWildcardKind::Expr(expr) => {
                            return Err(format!("`{expr}.*` is not supported"));
                        }
                    };
                    let entry_index = self.scope_entry(&qualifier)?;
                    self.push_table_outputs(entry_index, &mut outputs);
                }
                SelectItem::UnnamedExpr(expr) => {
                    self.expr(expr)?;
                    outputs.push(self.output(expr, None)?);
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    self.expr(expr)?;
                    outputs.push(self.output(expr, Some(alias))?);
                }
            }
        }
        Ok(outputs)
    }

    fn push_table_outputs(&self, entry_index: usize, outputs: &mut Vec<Output>) {
        let entry = &self.scope[entry_index];
        for (column_index, column) in entry.table.columns.iter().enumerate() {
            outputs.push(Output {
                sql_name: column.sql_name.clone(),
                rust_type: Some(column_type(column, entry.outer_nullable)),
                origin: entry
                    .table
                    .module()
                    .map(|module| (module.to_owned(), column_index)),
                nullable_origin: entry.outer_nullable,
                unknown_reason: None,
                name_error: None,
            });
        }
    }

    fn output(&self, expr: &Expr, alias: Option<&Ident>) -> Result<Output, String> {
        if let Some(reference) = self.column_reference(expr)? {
            let entry = &self.scope[reference.entry];
            let column = &entry.table.columns[reference.column];
            return Ok(Output {
                sql_name: alias
                    .map(ident_name)
                    .unwrap_or_else(|| column.sql_name.clone()),
                rust_type: Some(column_type(column, entry.outer_nullable)),
                origin: entry
                    .table
                    .module()
                    .map(|module| (module.to_owned(), reference.column)),
                nullable_origin: entry.outer_nullable,
                unknown_reason: None,
                name_error: None,
            });
        }

        let default_name = match expr {
            Expr::Function(function) if is_count(function) => "count".to_owned(),
            Expr::Function(function) => object_leaf_name(&function.name),
            _ => String::new(),
        };
        let rust_type = match self.value_type(expr)? {
            Some(value_type) if self.never_null(expr)? => Some(value_type),
            Some(value_type) => Some(format!("Option<{value_type}>")),
            None => None,
        };
        let sql_name = match alias {
            Some(alias) => ident_name(alias),
            None => default_name,
        };
        let name_error = sql_name.is_empty().then(|| {
            format!(
                "result expression `{expr}` needs an alias (`{expr} AS name`) and a `-- column: name <RustType>` override"
            )
        });
        Ok(Output {
            name_error,
            unknown_reason: rust_type.is_none().then(|| {
                format!(
                    "cannot infer the type of result column `{sql_name}` (`{expr}`); add `-- column: {sql_name} <RustType>`"
                )
            }),
            sql_name,
            rust_type,
            origin: None,
            nullable_origin: false,
        })
    }

    /// Whether `expr` is never NULL, as far as the engine can tell: a
    /// literal, `count`, `now()` and the other clock functions, `coalesce`
    /// with an argument that is never NULL, a `NOT NULL` column that is not
    /// on the outer side of a join, an `IS [NOT] NULL`-style test, `[NOT]
    /// EXISTS`, or an operator or cast none of whose operands can be NULL.
    fn never_null(&self, expr: &Expr) -> Result<bool, String> {
        if let Some(reference) = self.column_reference(expr)? {
            let entry = &self.scope[reference.entry];
            return Ok(!entry.outer_nullable && !entry.table.columns[reference.column].nullable);
        }
        Ok(match expr {
            Expr::Nested(inner)
            | Expr::Cast { expr: inner, .. }
            | Expr::UnaryOp { expr: inner, .. } => self.never_null(inner)?,
            Expr::Value(_) => is_non_null_literal(expr),
            Expr::TypedString(_) => true,
            Expr::Function(function) => match coalesce_arguments(function) {
                Some(arguments) => self.any_never_null(arguments)?,
                None => is_count(function) || clock_type(function).is_some(),
            },
            _ if is_is_test(expr) => true,
            Expr::Exists { .. } => true,
            Expr::BinaryOp { left, op, right } if is_typed_operator(op) => {
                self.all_never_null([&**left, &**right])?
            }
            Expr::Between {
                expr, low, high, ..
            } => self.all_never_null([&**expr, &**low, &**high])?,
            Expr::InList { expr, list, .. } => {
                self.never_null(expr)? && self.all_never_null(list)?
            }
            Expr::Like { expr, pattern, .. }
            | Expr::ILike { expr, pattern, .. }
            | Expr::SimilarTo { expr, pattern, .. } => {
                self.all_never_null([&**expr, &**pattern])?
            }
            Expr::Case {
                conditions,
                else_result: Some(else_result),
                ..
            } => {
                self.all_never_null(conditions.iter().map(|when| &when.result))?
                    && self.never_null(else_result)?
            }
            _ => false,
        })
    }

    fn all_never_null<'e>(
        &self,
        exprs: impl IntoIterator<Item = &'e Expr>,
    ) -> Result<bool, String> {
        for expr in exprs {
            if !self.never_null(expr)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn any_never_null(&self, exprs: Vec<&Expr>) -> Result<bool, String> {
        for expr in exprs {
            if self.never_null(expr)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The plain type under `value_type`: the column type a typed id wraps
    /// (`uuid::Uuid` for `MediaId`), or `value_type` itself.
    fn plain_type<'t>(&'t self, value_type: &'t str) -> &'t str {
        self.context
            .tables
            .iter()
            .flat_map(|table| &table.columns)
            .find(|column| {
                column
                    .typed_id
                    .as_ref()
                    .is_some_and(|id| id.name == value_type)
            })
            .map_or(value_type, |column| column.sqlx_type.as_str())
    }

    /// The type of `coalesce(…)`: the type its arguments share, as a `CASE`'s
    /// branches do. A literal takes the other arguments' type, as in
    /// PostgreSQL, so `coalesce(category_id, 0)` keeps the typed id; and
    /// arguments of one plain type under different typed ids (or none) take
    /// the plain type.
    fn coalesce_type(&self, arguments: &[&Expr]) -> Result<Option<String>, String> {
        let typed: Vec<&Expr> = arguments
            .iter()
            .copied()
            .filter(|argument| !is_non_null_literal(argument) && !is_null_literal(argument))
            .collect();
        let arguments = if typed.is_empty() {
            arguments
        } else {
            typed.as_slice()
        };
        let mut types = Vec::new();
        for argument in arguments
            .iter()
            .filter(|argument| !is_null_literal(argument))
        {
            match self.value_type(argument)? {
                Some(value_type) => types.push(value_type),
                None => return Ok(None),
            }
        }
        if let Some(shared) = shared_type(types.clone()) {
            return Ok(Some(shared));
        }
        Ok(shared_type(
            types
                .iter()
                .map(|value_type| self.plain_type(value_type).to_owned())
                .collect(),
        ))
    }

    /// The `RETURNS` type of the schema function `function` calls: an
    /// unqualified name matches the function in any schema, as a table name
    /// does. `None` for an unknown function, a type the engine doesn't map,
    /// or overloads that return different types.
    fn schema_function_type(&self, function: &Function) -> Option<String> {
        let parts: Vec<String> = function
            .name
            .0
            .iter()
            .filter_map(|part| match part {
                ObjectNamePart::Identifier(ident) => Some(ident_name(ident)),
                ObjectNamePart::Function(_) => None,
            })
            .collect();
        let (schema, leaf) = match parts.as_slice() {
            [.., schema, leaf] => (Some(schema.as_str()), leaf.as_str()),
            [leaf] => (None, leaf.as_str()),
            [] => return None,
        };
        let mut return_types = self
            .context
            .functions
            .iter()
            .filter(|candidate| {
                candidate.sql_name == leaf
                    && schema.is_none_or(|schema| {
                        candidate.schema.as_deref().unwrap_or("public") == schema
                    })
            })
            .map(|candidate| candidate.return_type.as_deref());
        let first = return_types.next()??;
        return_types
            .all(|return_type| return_type == Some(first))
            .then(|| first.to_owned())
    }

    /// The numeric type of an arithmetic operand: a column's plain type
    /// (never its typed id), or what [`Self::value_type`] infers.
    fn number_type(&self, expr: &Expr) -> Result<Option<Number>, String> {
        let value_type = match self.column_reference(expr)? {
            Some(reference) => Some(self.reference_column(&reference).sqlx_type.clone()),
            None => self
                .value_type(expr)?
                .map(|value_type| self.plain_type(&value_type).to_owned()),
        };
        Ok(value_type.as_deref().and_then(Number::of))
    }

    /// The Rust type of `expr`'s non-NULL values, where the engine infers one
    /// without looking at nullability: a column, a literal, a cast, `count`,
    /// a clock function, a comparison or boolean operator (`bool`),
    /// arithmetic (its operands' shared numeric type), `||` (text) or a `CASE`
    /// (the type its non-NULL branches share).
    fn value_type(&self, expr: &Expr) -> Result<Option<String>, String> {
        if let Some(reference) = self.column_reference(expr)? {
            let column = &self.scope[reference.entry].table.columns[reference.column];
            return Ok(Some(column.value_type().to_owned()));
        }
        Ok(match expr {
            Expr::Nested(inner) => self.value_type(inner)?,
            Expr::Value(value) => literal_type(&value.value),
            Expr::TypedString(typed) => Some(map_type(&typed.data_type, self.context.enums)?.1),
            Expr::Function(function) if is_count(function) => Some("i64".to_owned()),
            Expr::Function(function) => match coalesce_arguments(function) {
                Some(arguments) => self.coalesce_type(&arguments)?,
                None => clock_type(function)
                    .map(str::to_owned)
                    .or_else(|| self.schema_function_type(function)),
            },
            // A cast to the plain type a typed id wraps keeps the typed id:
            // `media_id::uuid` is still a `MediaId`.
            Expr::Cast {
                expr: inner,
                data_type,
                ..
            } => {
                let target = map_type(data_type, self.context.enums)?.1;
                match self.value_type(inner)? {
                    Some(value_type) if self.plain_type(&value_type) == target => Some(value_type),
                    _ => Some(target),
                }
            }
            Expr::Subquery(query) => self
                .subquery_types
                .get(&std::ptr::from_ref(query.as_ref()))
                .cloned()
                .flatten(),
            Expr::Exists { .. } | Expr::InSubquery { .. } => Some("bool".to_owned()),
            Expr::BinaryOp { op, .. } if is_boolean_operator(op) => Some("bool".to_owned()),
            Expr::BinaryOp {
                left,
                op: BinaryOperator::StringConcat,
                right,
            } => {
                let is_text = |value_type: &Option<String>| {
                    value_type.as_deref().is_none_or(|value_type| {
                        !value_type.starts_with("Vec<") && value_type != "serde_json::Value"
                    })
                };
                (is_text(&self.value_type(left)?) && is_text(&self.value_type(right)?))
                    .then(|| "String".to_owned())
            }
            Expr::Case {
                conditions,
                else_result,
                ..
            } => {
                let mut types = Vec::new();
                let branches = conditions
                    .iter()
                    .map(|when| &when.result)
                    .chain(else_result.as_deref());
                for branch in branches.filter(|branch| !is_null_literal(branch)) {
                    match self.value_type(branch)? {
                        Some(value_type) => types.push(value_type),
                        None => return Ok(None),
                    }
                }
                shared_type(types)
            }
            Expr::BinaryOp { left, op, right } if is_arithmetic(op) => {
                match (self.number_type(left)?, self.number_type(right)?) {
                    (Some(left), Some(right)) => Some(left.max(right).rust_type().to_owned()),
                    _ => None,
                }
            }
            Expr::UnaryOp {
                op: UnaryOperator::Minus | UnaryOperator::Plus,
                expr: inner,
            } => self
                .number_type(inner)?
                .map(|number| number.rust_type().to_owned()),
            _ if is_is_test(expr) => Some("bool".to_owned()),
            Expr::UnaryOp {
                op: UnaryOperator::Not,
                ..
            }
            | Expr::Between { .. }
            | Expr::InList { .. }
            | Expr::Like { .. }
            | Expr::ILike { .. }
            | Expr::SimilarTo { .. } => Some("bool".to_owned()),
            _ => None,
        })
    }
}

/// Comparisons, pattern matches and `AND`/`OR`: `bool` whatever the operands.
fn is_boolean_operator(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::PGRegexMatch
            | BinaryOperator::PGRegexIMatch
            | BinaryOperator::PGRegexNotMatch
            | BinaryOperator::PGRegexNotIMatch
            | BinaryOperator::PGLikeMatch
            | BinaryOperator::PGILikeMatch
            | BinaryOperator::PGNotLikeMatch
            | BinaryOperator::PGNotILikeMatch
    )
}

/// The type of a clock function (`now()`, `current_date`, ...), which is
/// never NULL.
fn clock_type(function: &Function) -> Option<&'static str> {
    let name = object_leaf_name(&function.name).to_ascii_lowercase();
    Some(match name.as_str() {
        "now"
        | "current_timestamp"
        | "transaction_timestamp"
        | "statement_timestamp"
        | "clock_timestamp" => "chrono::DateTime<chrono::Utc>",
        "current_date" => "chrono::NaiveDate",
        "localtimestamp" => "chrono::NaiveDateTime",
        "localtime" => "chrono::NaiveTime",
        _ => return None,
    })
}

/// The type PostgreSQL gives a literal: an integer that fits is `integer`
/// then `bigint`, any other number `numeric`; a string is `text`.
fn literal_type(value: &Value) -> Option<String> {
    Some(
        match value {
            Value::Number(number, _) if number.parse::<i32>().is_ok() => "i32",
            Value::Number(number, _) if number.parse::<i64>().is_ok() => "i64",
            Value::Number(..) => "rust_decimal::Decimal",
            Value::Boolean(_) => "bool",
            Value::SingleQuotedString(_)
            | Value::DollarQuotedString(_)
            | Value::EscapedStringLiteral(_)
            | Value::UnicodeStringLiteral(_) => "String",
            _ => return None,
        }
        .to_owned(),
    )
}

/// The arguments of a `coalesce(…)` call with at least one.
fn coalesce_arguments(function: &Function) -> Option<Vec<&Expr>> {
    if !object_leaf_name(&function.name).eq_ignore_ascii_case("coalesce") {
        return None;
    }
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    let arguments: Vec<&Expr> = list
        .args
        .iter()
        .map(|argument| match argument {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(argument)) => Some(argument),
            _ => None,
        })
        .collect::<Option<_>>()?;
    (!arguments.is_empty()).then_some(arguments)
}

/// A literal other than NULL or a placeholder, possibly signed (`-1`).
fn is_non_null_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Value(value) => !matches!(value.value, Value::Null | Value::Placeholder(_)),
        Expr::Nested(inner) => is_non_null_literal(inner),
        Expr::UnaryOp {
            op: UnaryOperator::Minus | UnaryOperator::Plus,
            expr,
        } => is_non_null_literal(expr),
        _ => false,
    }
}

pub(super) fn result_shape(
    def: &QueryDef,
    outputs: Vec<Output>,
    context: &Context<'_>,
) -> Result<ResultShape, String> {
    if let Some(name_error) = outputs.iter().find_map(|output| output.name_error.clone()) {
        return Err(name_error);
    }
    let mut seen = HashSet::new();
    for output in &outputs {
        if !seen.insert(output.sql_name.as_str()) {
            return Err(format!(
                "result column `{}` appears more than once; alias the duplicates",
                output.sql_name
            ));
        }
    }
    for override_ in &def.columns {
        if !seen.contains(override_.name.as_str()) {
            return Err(format!(
                "`-- column: {}` overrides a column the query does not return",
                override_.name
            ));
        }
    }

    if !def.mode.returns_rows() {
        return Ok(ResultShape::None);
    }
    if outputs.is_empty() {
        return Err(format!(
            "`:{}` returns rows, but the statement has no result columns; add RETURNING or use `:exec`",
            def.mode.keyword()
        ));
    }

    let overridden = |name: &str| def.columns.iter().find(|column| column.name == name);
    let mut columns = Vec::with_capacity(outputs.len());
    for output in &outputs {
        let rust_type = match (overridden(&output.sql_name), &output.rust_type) {
            (Some(override_), _) => override_.rust_type.clone(),
            (None, Some(rust_type)) => rust_type.clone(),
            (None, None) => {
                return Err(output.unknown_reason.clone().unwrap_or_else(|| {
                    format!(
                        "cannot infer the type of result column `{}`",
                        output.sql_name
                    )
                }));
            }
        };
        columns.push(OutputColumn {
            rust_name: naming::rust_identifier(&output.sql_name)?,
            sql_name: output.sql_name.clone(),
            rust_type,
        });
    }

    if let Some(model) = reusable_model(def, &outputs, context) {
        return Ok(model);
    }
    // A page holds rows: one column is still a `{Name}Row`.
    if let [column] = columns.as_slice()
        && def.mode != ReturnMode::Page
    {
        return Ok(ResultShape::Scalar(column.rust_type.clone()));
    }
    Ok(ResultShape::Row(columns))
}

/// The row struct of a generated resource, when the result is exactly that
/// table's columns in declaration order, unaltered by joins or overrides.
fn reusable_model(
    def: &QueryDef,
    outputs: &[Output],
    context: &Context<'_>,
) -> Option<ResultShape> {
    if !def.columns.is_empty() || outputs.len() < 2 {
        return None;
    }
    let (module, _) = outputs[0].origin.as_ref()?;
    let table = context
        .tables
        .iter()
        .find(|table| &table.module_name == module)?;
    let is_whole_table = outputs.len() == table.columns.len()
        && outputs.iter().enumerate().all(|(index, output)| {
            !output.nullable_origin
                && output.sql_name == table.columns[index].sql_name
                && output.origin.as_ref() == Some(&(module.clone(), index))
        });
    (is_whole_table && context.generated_modules.contains(module)).then(|| ResultShape::Model {
        module: table.module_name.clone(),
        struct_name: table.struct_name.clone(),
    })
}

/// The Rust type of a result column drawn directly from `column`: its field
/// type (the typed id, when it has one), optional on the outer side of a join.
fn column_type(column: &Column, outer_nullable: bool) -> String {
    if outer_nullable && !column.nullable {
        format!("Option<{}>", column.value_type())
    } else {
        column.field_type()
    }
}

fn is_count(function: &sqlparser::ast::Function) -> bool {
    object_leaf_name(&function.name).eq_ignore_ascii_case("count")
}

/// An operator whose result the engine types: NULL only when an operand is.
fn is_typed_operator(op: &BinaryOperator) -> bool {
    is_boolean_operator(op) || is_arithmetic(op) || *op == BinaryOperator::StringConcat
}

fn is_arithmetic(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
    )
}

/// A numeric type, ordered so that the larger of two operands' types is
/// their result's: integers widen, `numeric` wins over integers and floats
/// over `numeric`, as PostgreSQL's implicit casts do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Number {
    SmallInt,
    Integer,
    BigInt,
    Numeric,
    Real,
    Double,
}

impl Number {
    fn of(rust_type: &str) -> Option<Number> {
        Some(match rust_type {
            "i16" => Number::SmallInt,
            "i32" => Number::Integer,
            "i64" => Number::BigInt,
            "rust_decimal::Decimal" => Number::Numeric,
            "f32" => Number::Real,
            "f64" => Number::Double,
            _ => return None,
        })
    }

    fn rust_type(self) -> &'static str {
        match self {
            Number::SmallInt => "i16",
            Number::Integer => "i32",
            Number::BigInt => "i64",
            Number::Numeric => "rust_decimal::Decimal",
            Number::Real => "f32",
            Number::Double => "f64",
        }
    }
}

fn is_null_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Value(value) => value.value == Value::Null,
        Expr::Nested(inner) => is_null_literal(inner),
        _ => false,
    }
}

/// The type `CASE` branches of `types` share: their one type, or the widest
/// of their numeric types. `None` when they have none in common.
fn shared_type(types: Vec<String>) -> Option<String> {
    let first = types.first()?;
    if types.iter().all(|value_type| value_type == first) {
        return Some(first.clone());
    }
    types
        .iter()
        .map(|value_type| Number::of(value_type))
        .collect::<Option<Vec<Number>>>()?
        .into_iter()
        .max()
        .map(|number| number.rust_type().to_owned())
}

/// `IS [NOT] NULL`, `IS [NOT] TRUE`/`FALSE`/`UNKNOWN` and `IS [NOT]
/// DISTINCT FROM`: `bool`, and never NULL.
fn is_is_test(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IsNull(_)
            | Expr::IsNotNull(_)
            | Expr::IsTrue(_)
            | Expr::IsNotTrue(_)
            | Expr::IsFalse(_)
            | Expr::IsNotFalse(_)
            | Expr::IsUnknown(_)
            | Expr::IsNotUnknown(_)
            | Expr::IsDistinctFrom(..)
            | Expr::IsNotDistinctFrom(..)
    )
}
