use chrono::{DateTime, NaiveDate, Utc};
use optional_field::Field;
use pg_query_mapper::PgQueryMapper;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio_postgres::NoTls;
use uuid::Uuid;

#[derive(PgQueryMapper, Debug, PartialEq)]
#[pg_mapper(alias_prefix = "c_")]
pub struct Company {
    pub id: Uuid,
    pub company_no: String,
    pub name: Field<String>,
    pub description: Field<String>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Profile {
    pub birth_date: Field<NaiveDate>,
    pub salary: Field<Decimal>,
}

#[derive(Default, Debug, PartialEq)]
pub struct Address {
    pub user_name: String,
    pub street: String,
    pub city: String,
}

#[derive(PgQueryMapper, Debug, PartialEq)]
#[pg_mapper(alias_prefix = "u_")]
pub struct User {
    pub id: Uuid,
    pub created_at: Field<DateTime<Utc>>,

    #[pg_mapper(rename = "user_name")]
    pub name: String,

    #[pg_mapper(flatten)]
    pub company: Field<Company>,

    #[pg_mapper(json)]
    pub profile: Field<Profile>,

    #[pg_mapper(skip)]
    pub addresses: Vec<Address>,
}

#[tokio::test]
async fn test_mapper_integration() {
    let node = Postgres::default().start().await.unwrap();
    let port = node.get_host_port_ipv4(5432).await.unwrap();

    let db_url = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let (client, connection) = tokio_postgres::connect(&db_url, NoTls).await.unwrap();

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    client
        .execute(
            "CREATE TABLE companies (
            id UUID PRIMARY KEY,
            company_no VARCHAR NOT NULL,
            name VARCHAR,
            description VARCHAR
        )",
            &[],
        )
        .await
        .unwrap();

    client
        .execute(
            "CREATE TABLE users (
            id UUID PRIMARY KEY,
            created_at TIMESTAMPTZ,
            name VARCHAR NOT NULL,
            company_id UUID REFERENCES companies(id),
            profile JSONB
        )",
            &[],
        )
        .await
        .unwrap();

    let company_id = Uuid::new_v4();
    client.execute(
        "INSERT INTO companies (id, company_no, name, description) VALUES ($1, 'CORP-A', 'Corporation A', 'A test corp')",
        &[&company_id],
    ).await.unwrap();

    let alice_id = Uuid::new_v4();
    let alice_created_at = Utc::now();
    let profile_json = serde_json::json!({
        "birth_date": "1990-01-01",
        "salary": "120000.50"
    });

    client.execute(
        "INSERT INTO users (id, created_at, name, company_id, profile) VALUES ($1, $2, 'Alice', $3, $4)",
        &[&alice_id, &alice_created_at, &company_id, &profile_json],
    ).await.unwrap();

    let bob_id = Uuid::new_v4();
    let bob_created_at = Utc::now();

    client.execute(
        "INSERT INTO users (id, created_at, name, company_id, profile) VALUES ($1, $2, 'Bob', NULL, NULL)",
        &[&bob_id, &bob_created_at],
    ).await.unwrap();

    let sql = "
        SELECT 
            u.id AS u_id,
            u.created_at AS u_created_at,
            u.name AS user_name,
            u.profile AS u_profile,
            c.id AS c_id,
            c.company_no AS c_company_no,
            c.name AS c_name,
            c.description AS c_description
        FROM users u
        LEFT JOIN companies c ON u.company_id = c.id
        ORDER BY u.name ASC
    ";

    let stmt = client.prepare(sql).await.unwrap();
    let mapper = UserMapper::new(stmt.columns()).expect("Failed to initialize UserMapper");

    let rows = client.query(&stmt, &[]).await.unwrap();
    assert_eq!(rows.len(), 2);

    let alice = mapper.map(&rows[0]).expect("Failed to map Alice");
    let bob = mapper.map(&rows[1]).expect("Failed to map Bob");

    // Assertions for Alice
    assert_eq!(alice.id, alice_id);
    assert_eq!(alice.name, "Alice");
    // PostgreSQL microseconds truncation might cause inequality if we check exact timestamps
    // so we'll just check it's present
    assert!(matches!(alice.created_at, Field::Present(_)));

    match &alice.company {
        Field::Present(Some(company)) => {
            assert_eq!(company.id, company_id);
            assert_eq!(company.company_no, "CORP-A");
            assert_eq!(
                company.name,
                Field::Present(Some("Corporation A".to_string()))
            );
            assert_eq!(
                company.description,
                Field::Present(Some("A test corp".to_string()))
            );
        }
        _ => panic!("Expected company for Alice"),
    }

    match &alice.profile {
        Field::Present(Some(profile)) => {
            assert_eq!(
                profile.birth_date,
                Field::Present(Some(NaiveDate::from_ymd_opt(1990, 1, 1).unwrap()))
            );
            assert_eq!(
                profile.salary,
                Field::Present(Some(Decimal::from_str("120000.50").unwrap()))
            );
        }
        _ => panic!("Expected profile for Alice"),
    }

    assert!(alice.addresses.is_empty());

    // Assertions for Bob
    assert_eq!(bob.id, bob_id);
    assert_eq!(bob.name, "Bob");
    assert!(matches!(bob.created_at, Field::Present(_)));
    assert_eq!(bob.company, Field::Present(None));
    assert_eq!(bob.profile, Field::Present(None));
    assert!(bob.addresses.is_empty());
}
