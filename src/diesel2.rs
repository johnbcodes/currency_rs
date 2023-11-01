#[cfg(feature = "db-diesel2-sqlite")]
pub mod sqlite {
    use crate::Currency;
    use diesel::{
        deserialize::{self, FromSql},
        sqlite::Sqlite,
        serialize::{self, Output, ToSql},
        sql_types::Double,
    };

    pub mod sql_types {
        #[derive(diesel::sql_types::SqlType)]
        #[diesel(sqlite_type(name = "Double"))]
        pub struct Currency;
    }

    impl ToSql<sql_types::Currency, Sqlite> for Currency {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
            <f64 as ToSql<Double, Sqlite>>::to_sql(&self.value, out)
        }
    }

    impl FromSql<sql_types::Currency, Sqlite> for Currency {
        fn from_sql(value: diesel::sqlite::SqliteValue) -> deserialize::Result<Self> {
            let value = <f64 as FromSql<Double, Sqlite>>::from_sql(value)?;
            Ok(Currency::new_float(value, None))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use diesel::connection::SimpleConnection;
        use diesel::deserialize::QueryableByName;
        use diesel::dsl::*;
        use diesel::prelude::*;
        use diesel::row::NamedRow;
        use diesel::sql_query;
        use diesel::sql_types::Text;

        struct Test {
            value: Currency,
        }

        struct NullableTest {
            value: Option<Currency>,
        }

        fn get_sqlite_url() -> String {
            std::env::var("SQLITE_URL").unwrap_or(String::from(":memory:"))
        }

        pub static TEST_DECIMALS: &[(u32, u32, &str, &str)] = &[
            // precision, scale, sent, expected
            (1, 0, "1", "1.00"),
            (6, 2, "1", "1.00"),
            (6, 2, "9999.99", "9999.99"),
            (35, 6, "3950.123456", "3950.12"),
            (10, 2, "3950.123456", "3950.12"),
            (35, 6, "3950", "3950.00"),
            (4, 0, "3950", "3950.00"),
            (35, 6, "0.1", "0.10"),
            (35, 6, "0.01", "0.01"),
            (35, 6, "0.001", "0.00"),
            (35, 6, "0.0001", "0.00"),
            (35, 6, "0.00001", "0.00"),
            (35, 6, "0.000001", "0.00"),
            (35, 6, "1", "1.00"),
            (35, 6, "-100", "-100.00"),
            (35, 6, "-123.456", "-123.46"),
            (35, 6, "119996.25", "119996.25"),
            (35, 6, "1000000", "1000000.00"),
            (35, 6, "9999999.99999", "10000000.00"),
            (35, 6, "12340.56789", "12340.57"),
        ];

        table! {
            use diesel::sql_types::*;
            use super::super::sql_types::Currency;

            currencies {
                id -> Integer,
                currency -> Currency,
            }
        }

        impl QueryableByName<Sqlite> for Test {
            fn build<'a>(row: &impl NamedRow<'a, Sqlite>) -> deserialize::Result<Self> {
                let value = NamedRow::get(row, "value")?;
                Ok(Test { value })
            }
        }

        impl QueryableByName<Sqlite> for NullableTest {
            fn build<'a>(row: &impl NamedRow<'a, Sqlite>) -> deserialize::Result<Self> {
                let value = NamedRow::get(row, "value")?;
                Ok(NullableTest { value })
            }
        }

        #[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
        #[diesel(table_name = currencies)]
        struct Currencies {
            id: i32,
            currency: Currency,
        }

        #[test]
        fn test_null() {
            let mut connection = SqliteConnection::establish(&get_sqlite_url()).expect("Establish connection");

            // Test NULL
            let items: Vec<NullableTest> = sql_query("SELECT CAST(NULL AS DECIMAL) AS value")
                .load(&mut connection)
                .expect("Unable to query value");
            let result = &items.first().unwrap().value;
            assert!(result.is_none());
        }

        #[test]
        fn read_numeric_type() {
            let mut connection = SqliteConnection::establish(&get_sqlite_url()).expect("Establish connection");
            for &(precision, scale, sent, expected) in TEST_DECIMALS.iter() {
                let items: Vec<Test> = sql_query(format!(
                    "SELECT CAST('{}' AS DECIMAL({}, {})) AS value",
                    sent, precision, scale
                ))
                    .load(&mut connection)
                    .expect("Unable to query value");
                assert_eq!(
                    expected,
                    items.first().unwrap().value.to_string(),
                    "DECIMAL({}, {}) sent: {}",
                    precision,
                    scale,
                    sent
                );
            }
        }

        #[test]
        fn write_numeric_type() {
            let mut connection = SqliteConnection::establish(&get_sqlite_url()).expect("Establish connection");
            for &(precision, scale, sent, expected) in TEST_DECIMALS.iter() {
                let items: Vec<Test> =
                    sql_query(format!("SELECT CAST(? AS DECIMAL({}, {})) AS value", precision, scale))
                        .bind::<Text, _>(sent)
                        .load(&mut connection)
                        .expect("Unable to query value");
                assert_eq!(
                    expected,
                    items.first().unwrap().value.to_string(),
                    "DECIMAL({}, {}) sent: {}",
                    precision,
                    scale,
                    sent
                );
            }
        }

        #[test]
        fn custom_types_round_trip() {
            use crate::diesel2::sqlite::tests::currencies::dsl::currencies;

            let data = vec![
                Currencies { id: 1, currency: Currency::new_float(0.10, None) },
                Currencies { id: 2, currency: Currency::new_float(200f64, None) },
            ];
            let mut connection = SqliteConnection::establish(&get_sqlite_url()).expect("Establish connection");
            connection
                .batch_execute(
                    r#"
                    CREATE TABLE currencies (
                        id SERIAL PRIMARY KEY,
                        currency double NOT NULL
                    );
                "#,
                )
                .unwrap();

            insert_into(currencies)
                .values(&data)
                .execute(&mut connection)
                .unwrap();

            let inserted: Vec<Currencies> = currencies
                .get_results(&mut connection)
                .unwrap();

            assert_eq!(data, inserted);
        }
    }
}