//! `lib/src/models.dart`: the declarations [`Models`](super::models::Models)
//! built, written out.

use super::models::{Class, Decl, Field};
use super::names::{doc, string};

/// The text of `lib/src/models.dart` (after the header).
pub(super) fn file(decls: &[Decl]) -> String {
    let mut out = String::new();
    let tristate = decls.iter().any(|decl| {
        matches!(decl, Decl::Class(class) if class.fields.iter().any(|field| field.tristate))
    });
    if tristate {
        out.push_str("\nimport 'runtime.dart';\n");
    }
    for decl in decls {
        out.push('\n');
        out.push_str(&match decl {
            Decl::Class(class) => class_text(class),
            Decl::Enum { name, values } => enum_text(name, values),
            Decl::Union { name, variants } => union_text(name, variants),
            Decl::Alias { name, ty } => {
                format!(
                    "/// The `{name}` schema.\ntypedef {name} = {};\n",
                    ty.dart()
                )
            }
        });
    }
    out
}

/// A field's Dart type.
fn field_type(field: &Field) -> String {
    if field.tristate {
        format!("Nullable<{}>?", field.ty.non_null().dart())
    } else {
        field.ty.dart()
    }
}

fn class_text(class: &Class) -> String {
    let name = &class.name;
    let implements = if class.implements.is_empty() {
        String::new()
    } else {
        format!(" implements {}", class.implements.join(", "))
    };
    let mut out = format!("/// The `{name}` model.\nfinal class {name}{implements} {{\n");
    out.push_str(&format!(
        "  /// Makes one from its fields.\n  const {name}({{\n"
    ));
    for field in &class.fields {
        let required = if field.required { "required " } else { "" };
        out.push_str(&format!("    {required}this.{},\n", field.name));
    }
    out.push_str("  });\n\n");
    out.push_str(&format!(
        "  /// Reads one from its JSON.\n  factory {name}.fromJson(Map<String, Object?> json) => {name}(\n"
    ));
    for field in &class.fields {
        let value = format!("json[{}]", string(&field.key));
        let read = if field.tristate {
            format!(
                "json.containsKey({}) ? Nullable({}) : null",
                string(&field.key),
                field.ty.decode(&value, 0)
            )
        } else {
            field.ty.decode(&value, 0)
        };
        out.push_str(&format!("    {}: {read},\n", field.name));
    }
    out.push_str("  );\n");
    for field in &class.fields {
        out.push('\n');
        out.push_str(&doc(field.doc.as_deref(), "  "));
        out.push_str(&format!("  final {} {};\n", field_type(field), field.name));
    }
    out.push_str("\n  /// Its JSON.\n");
    if !class.implements.is_empty() {
        out.push_str("  @override\n");
    }
    out.push_str("  Map<String, Object?> toJson() => {\n");
    for field in &class.fields {
        let key = string(&field.key);
        if field.tristate {
            out.push_str(&format!(
                "    if ({} case final set?) {key}: {},\n",
                field.name,
                field.ty.encode("set.value", 0)
            ));
        } else if field.required {
            out.push_str(&format!(
                "    {key}: {},\n",
                field.ty.encode(&field.name, 0)
            ));
        } else {
            out.push_str(&format!(
                "    {key}: ?{},\n",
                field.ty.encode(&field.name, 0)
            ));
        }
    }
    out.push_str("  };\n}\n");
    out
}

fn enum_text(name: &str, values: &[(String, String)]) -> String {
    let mut out = format!("/// The `{name}` values.\nenum {name} {{\n");
    let list: Vec<String> = values
        .iter()
        .map(|(ident, wire)| format!("  {ident}({})", string(wire)))
        .collect();
    out.push_str(&list.join(",\n"));
    out.push_str(&format!(
        ";\n\n  const {name}(this.value);\n\n  /// Its value on the wire.\n  final String value;\n\n  \
         /// Reads one from its JSON.\n  static {name} fromJson(String json) => values.firstWhere(\n    \
         (v) => v.value == json,\n    orElse: () => throw FormatException({}, json),\n  );\n\n  \
         /// Its JSON.\n  String toJson() => value;\n}}\n",
        string(&format!("not a {name}"))
    ));
    out
}

fn union_text(name: &str, variants: &[(String, Vec<String>)]) -> String {
    let mut out = format!(
        "/// The `{name}` model: one of its variants.\nsealed class {name} {{\n  \
         /// Reads one from its JSON: the variant whose required keys it holds.\n  \
         factory {name}.fromJson(Map<String, Object?> json) {{\n"
    );
    let mut exhaustive = false;
    for (class, keys) in variants {
        if keys.is_empty() {
            out.push_str(&format!("    return {class}.fromJson(json);\n"));
            exhaustive = true;
            break;
        }
        let test: Vec<String> = keys
            .iter()
            .map(|key| format!("json.containsKey({})", string(key)))
            .collect();
        out.push_str(&format!(
            "    if ({}) {{\n      return {class}.fromJson(json);\n    }}\n",
            test.join(" && ")
        ));
    }
    if !exhaustive {
        out.push_str(&format!(
            "    throw FormatException({}, json);\n",
            string(&format!("no variant of {name}"))
        ));
    }
    out.push_str("  }\n\n  /// Its JSON.\n  Map<String, Object?> toJson();\n}\n");
    out
}
