//! Value objects over several columns, and setters that transform what
//! they write.

use super::*;

#[derive(Debug, Clone, PartialEq)]
struct Address {
    street: String,
    city: String,
}

impl From<(String, String)> for Address {
    fn from((street, city): (String, String)) -> Self {
        Self { street, city }
    }
}

impl From<Address> for (String, String) {
    fn from(address: Address) -> Self {
        (address.street, address.city)
    }
}

/// The write-time transform: stands in for a password hash.
fn shout(value: String) -> String {
    value.to_uppercase()
}

#[derive(Debug, Clone, PartialEq, Model)]
#[rok(table = "shops", value_object(address: Address = (street, city)))]
struct Shop {
    #[rok(generated)]
    id: i64,
    street: String,
    city: String,
    #[rok(set_with = shout)]
    code: String,
}

#[test]
fn a_value_object_reads_and_writes_its_columns() {
    let mut shop = Shop {
        id: 1,
        street: "Main St 1".into(),
        city: "Oslo".into(),
        code: String::new(),
    };
    assert_eq!(
        shop.address(),
        Address {
            street: "Main St 1".into(),
            city: "Oslo".into()
        }
    );
    shop.set_address(Address {
        street: "Side St 2".into(),
        city: "Bergen".into(),
    });
    assert_eq!(
        (shop.street.as_str(), shop.city.as_str()),
        ("Side St 2", "Bergen")
    );
    shop.set_code("abc".into());
    assert_eq!(shop.code, "ABC");
}
