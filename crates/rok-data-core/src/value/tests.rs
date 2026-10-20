//! The value layer's own tests.

use super::*;

#[test]
fn conversions_keep_types() {
    assert_eq!(Value::from(1_i32), Value::I32(Some(1)));
    assert_eq!(Value::from("a"), Value::String(Some("a".into())));
    assert_eq!(Value::from(None::<i64>), Value::I64(None));
    assert_eq!(Value::from(&Some(true)), Value::Bool(Some(true)));
    assert!(Value::from(None::<String>).is_null());
    assert!(!Value::from(0_u8).is_null());
}
