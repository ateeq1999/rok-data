//! The Dart type of a value, and the expressions that read it from its
//! JSON and write it back.

/// The Dart type of a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ty {
    /// `String`.
    String,
    /// `int`.
    Int,
    /// `double`.
    Double,
    /// `bool`.
    Bool,
    /// `Object?`: any JSON value, `null` included.
    Any,
    /// `List<T>`.
    List(Box<Ty>),
    /// `Map<String, T>`.
    Map(Box<Ty>),
    /// A generated class (or sealed union), read from a JSON object.
    Class(String),
    /// A generated enum, read from a JSON string.
    Enum(String),
    /// `T?`.
    Nullable(Box<Ty>),
}

impl Ty {
    /// The type, or `null`: `T?`. `Object?` and `T?` already are.
    pub fn nullable(self) -> Ty {
        match self {
            Ty::Any | Ty::Nullable(_) => self,
            other => Ty::Nullable(Box::new(other)),
        }
    }

    /// The type without its `?`.
    pub fn non_null(&self) -> &Ty {
        match self {
            Ty::Nullable(inner) => inner,
            other => other,
        }
    }

    /// Whether `null` is one of its values.
    pub fn is_nullable(&self) -> bool {
        matches!(self, Ty::Any | Ty::Nullable(_))
    }

    /// Whether it names a generated class or enum, at any depth.
    pub fn names_a_model(&self) -> bool {
        match self {
            Ty::Class(_) | Ty::Enum(_) => true,
            Ty::List(inner) | Ty::Map(inner) | Ty::Nullable(inner) => inner.names_a_model(),
            Ty::String | Ty::Int | Ty::Double | Ty::Bool | Ty::Any => false,
        }
    }

    /// Whether it is a query or path value: a scalar or an enum.
    pub fn is_scalar(&self) -> bool {
        matches!(
            self.non_null(),
            Ty::String | Ty::Int | Ty::Double | Ty::Bool | Ty::Enum(_)
        )
    }

    /// Its Dart text.
    pub fn dart(&self) -> String {
        match self {
            Ty::String => "String".to_owned(),
            Ty::Int => "int".to_owned(),
            Ty::Double => "double".to_owned(),
            Ty::Bool => "bool".to_owned(),
            Ty::Any => "Object?".to_owned(),
            Ty::List(inner) => format!("List<{}>", inner.dart()),
            Ty::Map(inner) => format!("Map<String, {}>", inner.dart()),
            Ty::Class(name) | Ty::Enum(name) => name.clone(),
            Ty::Nullable(inner) => format!("{}?", inner.dart()),
        }
    }

    /// The expression reading a value of this type from `json`, an
    /// `Object?` expression evaluated at most twice; `depth` names the
    /// closures' parameters apart.
    pub fn decode(&self, json: &str, depth: usize) -> String {
        let v = parameter(depth);
        match self {
            Ty::String | Ty::Bool => format!("{json} as {}", self.dart()),
            Ty::Int => format!("({json} as num).toInt()"),
            Ty::Double => format!("({json} as num).toDouble()"),
            Ty::Any => json.to_owned(),
            Ty::List(inner) if **inner == Ty::Any => format!("{json} as List<Object?>"),
            Ty::List(inner) => format!(
                "({json} as List<Object?>).map(({v}) => {}).toList()",
                inner.decode(&v, depth + 1)
            ),
            Ty::Map(inner) if **inner == Ty::Any => format!("{json} as Map<String, Object?>"),
            Ty::Map(inner) => format!(
                "({json} as Map<String, Object?>).map((k, {v}) => MapEntry(k, {}))",
                inner.decode(&v, depth + 1)
            ),
            Ty::Class(name) => format!("{name}.fromJson({json} as Map<String, Object?>)"),
            Ty::Enum(name) => format!("{name}.fromJson({json} as String)"),
            Ty::Nullable(inner) => match &**inner {
                Ty::String | Ty::Bool => format!("{json} as {}", self.dart()),
                Ty::Int => format!("({json} as num?)?.toInt()"),
                Ty::Double => format!("({json} as num?)?.toDouble()"),
                Ty::List(item) if **item == Ty::Any => format!("{json} as List<Object?>?"),
                Ty::List(item) => format!(
                    "({json} as List<Object?>?)?.map(({v}) => {}).toList()",
                    item.decode(&v, depth + 1)
                ),
                Ty::Map(value) if **value == Ty::Any => {
                    format!("{json} as Map<String, Object?>?")
                }
                Ty::Map(value) => format!(
                    "({json} as Map<String, Object?>?)?.map((k, {v}) => MapEntry(k, {}))",
                    value.decode(&v, depth + 1)
                ),
                other => format!("{json} == null ? null : {}", other.decode(json, depth)),
            },
        }
    }

    /// Whether its JSON is the value itself.
    fn encodes_as_itself(&self) -> bool {
        match self {
            Ty::String | Ty::Int | Ty::Double | Ty::Bool | Ty::Any => true,
            Ty::List(inner) | Ty::Map(inner) | Ty::Nullable(inner) => inner.encodes_as_itself(),
            Ty::Class(_) | Ty::Enum(_) => false,
        }
    }

    /// The expression writing `value`, of this type, as JSON.
    pub fn encode(&self, value: &str, depth: usize) -> String {
        if self.encodes_as_itself() {
            return value.to_owned();
        }
        let v = parameter(depth);
        let (inner, access) = match self {
            Ty::Nullable(inner) => (&**inner, "?."),
            other => (other, "."),
        };
        match inner {
            Ty::List(item) => format!(
                "{value}{access}map(({v}) => {}).toList()",
                item.encode(&v, depth + 1)
            ),
            Ty::Map(item) => format!(
                "{value}{access}map((k, {v}) => MapEntry(k, {}))",
                item.encode(&v, depth + 1)
            ),
            _ => format!("{value}{access}toJson()"),
        }
    }
}

/// The parameter of a closure at `depth`: `v`, `v1`, `v2`...
fn parameter(depth: usize) -> String {
    if depth == 0 {
        "v".to_owned()
    } else {
        format!("v{depth}")
    }
}

#[cfg(test)]
mod tests {
    use super::Ty;

    fn list(inner: Ty) -> Ty {
        Ty::List(Box::new(inner))
    }

    #[test]
    fn decodes_scalars_models_and_collections() {
        let json = "json['a']";
        assert_eq!(Ty::String.decode(json, 0), "json['a'] as String");
        assert_eq!(
            Ty::Int.nullable().decode(json, 0),
            "(json['a'] as num?)?.toInt()"
        );
        assert_eq!(
            Ty::Class("User".into()).nullable().decode(json, 0),
            "json['a'] == null ? null : User.fromJson(json['a'] as Map<String, Object?>)"
        );
        assert_eq!(
            list(Ty::Enum("Role".into())).decode(json, 0),
            "(json['a'] as List<Object?>).map((v) => Role.fromJson(v as String)).toList()"
        );
        assert_eq!(
            Ty::Map(Box::new(Ty::String)).decode(json, 0),
            "(json['a'] as Map<String, Object?>).map((k, v) => MapEntry(k, v as String))"
        );
    }

    #[test]
    fn encodes_models_and_leaves_plain_values_alone() {
        assert_eq!(list(Ty::String).encode("tags", 0), "tags");
        assert_eq!(
            Ty::Enum("Role".into()).nullable().encode("role", 0),
            "role?.toJson()"
        );
        assert_eq!(
            list(Ty::Class("Item".into())).nullable().encode("items", 0),
            "items?.map((v) => v.toJson()).toList()"
        );
    }
}
