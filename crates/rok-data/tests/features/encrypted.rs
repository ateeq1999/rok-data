//! `Encrypted` columns: plain text in the field, ciphertext in the table.

#[cfg(feature = "encryption")]
mod with_encryption {
    use super::super::*;
    use rok_data::Encrypted;

    #[derive(Debug, Clone, PartialEq, Model)]
    struct Account {
        #[rok(generated)]
        id: i64,
        owner: String,
        iban: Encrypted,
        note: Option<Encrypted>,
    }

    #[tokio::test]
    async fn an_encrypted_column_round_trips_and_stores_ciphertext() {
        let Some(db) = setup().await else { return };
        let _ = rok_data::set_encryption_key(&[3; 32]);
        db.execute(
            "CREATE TEMP TABLE accounts (id BIGSERIAL PRIMARY KEY, owner TEXT NOT NULL, iban TEXT NOT NULL, note TEXT)",
        )
        .await
        .unwrap();
        let stored = Account {
            id: 0,
            owner: "ann".into(),
            iban: "DE89370400440532013000".into(),
            note: None,
        }
        .insert(&db)
        .await
        .unwrap();
        assert_eq!(stored.iban.as_str(), "DE89370400440532013000");
        assert_eq!(stored.note, None);
        let found = Account::find(&db, stored.id)
            .await
            .unwrap()
            .expect("stored");
        assert_eq!(found.iban.as_str(), "DE89370400440532013000");
        let raw: Vec<(String,)> = rok_data::raw("SELECT iban FROM accounts")
            .fetch_all(&db)
            .await
            .unwrap();
        assert!(!raw[0].0.contains("DE89"), "the table holds ciphertext");
    }
}
