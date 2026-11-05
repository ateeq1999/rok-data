//! `CREATE FUNCTION` and `DROP FUNCTION`: the signatures query analysis
//! types a call to a schema function with. Function bodies are not read.

use sqlparser::ast::{ArgMode, CreateFunction, FunctionDesc, OperateFunctionArg};

use super::enums::EnumRegistry;
use super::names::{normalize_schema, object_leaf_name, object_schema_name};
use super::types::map_type_in;
use crate::schema::SqlFunction;

/// Records `create`, replacing a function of the same name and argument
/// types (`CREATE OR REPLACE`, or a redefinition).
pub(super) fn define_function(
    functions: &mut Vec<SqlFunction>,
    create: &CreateFunction,
    enums: &EnumRegistry,
) {
    let function = SqlFunction {
        schema: normalize_schema(object_schema_name(&create.name)),
        sql_name: object_leaf_name(&create.name),
        arg_types: create.args.as_deref().map(arg_types).unwrap_or_default(),
        returns: create.return_type.as_ref().map(ToString::to_string),
        return_type: create
            .return_type
            .as_ref()
            .and_then(|data_type| map_type_in(data_type, enums).ok())
            .map(|(_, sqlx_type)| sqlx_type),
    };
    functions.retain(|existing| {
        !(existing.schema == function.schema
            && existing.sql_name == function.sql_name
            && existing.arg_types == function.arg_types)
    });
    functions.push(function);
}

/// Forgets the functions a `DROP FUNCTION` names: every overload of a name
/// given without arguments, or the one whose argument types match.
pub(super) fn drop_functions(functions: &mut Vec<SqlFunction>, dropped: &[FunctionDesc]) {
    for desc in dropped {
        let schema = normalize_schema(object_schema_name(&desc.name));
        let name = object_leaf_name(&desc.name);
        let args = desc.args.as_deref().map(arg_types);
        functions.retain(|function| {
            !(function.schema == schema
                && function.sql_name == name
                && args.as_ref().is_none_or(|args| *args == function.arg_types))
        });
    }
}

/// The types of a function's input arguments, as written: `OUT` arguments
/// are not part of its signature.
fn arg_types(args: &[OperateFunctionArg]) -> Vec<String> {
    args.iter()
        .filter(|arg| !matches!(arg.mode, Some(ArgMode::Out)))
        .map(|arg| arg.data_type.to_string())
        .collect()
}
