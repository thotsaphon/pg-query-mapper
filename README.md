# pg-query-mapper 🚀

A high-performance, zero-overhead procedural macro mapping `tokio-postgres` Rows to Rust structs using a pre-computed column mapping pattern.

---

## 🎯 The Core Concept: Pre-computed Mapping

Standard ORMs or helper mapping libraries typically resolve database columns to struct fields by string lookup (e.g., `row.try_get("column_name")`) on **every single row**. When query results contain thousands of rows, these string lookups and hash table lookups introduce unnecessary overhead.

`pg-query-mapper` solves this with the **Pre-computed Mapper Pattern**:
1. Applying `#[derive(PgQueryMapper)]` to a struct `Foo` generates a companion struct `FooMapper`.
2. When you prepare a statement, you instantiate `FooMapper::new(statement.columns())`.
3. The mapper resolves and caches all column index mappings **exactly once**.
4. Subsequent row iterations execute in **$O(1)$ time** by fetching fields directly using their pre-resolved `usize` indices, eliminating string lookups completely.

---

## 📦 Installation

Add `pg-query-mapper` to your `Cargo.toml`:

```toml
[dependencies]
pg-query-mapper = "0.1.0"
tokio-postgres = { version = "0.7", features = ["with-serde_json-1"] }
serde = { version = "1.0", features = ["derive"] } # For JSON mapping
# Use the stable version from crates.io
optional-field = "0.1" # For optional fields
```

**Using the Extended Feature (Postgres & Display)**

If you need the extended features (Postgres FromSql/ToSql support and Display implementation for Field<T>), you can use the development branch from my fork:
```toml
[dependencies]
# ... other dependencies
optional-field = { git = "https://github.com/thotsaphon/optional-field", branch = "next" }
```
**Features provided by this branch:**

*   **Postgres Support**: Implements `postgres_types::FromSql` and `ToSql` for `Field<T>`, allowing seamless integration with tokio-postgres rows (e.g., `row.get::<_, Field<Uuid>>(0)`).
*   **Logging**: Implements `std::fmt::Display` for `Field<T>` to make debugging and logging easier.

---

## 🛠️ Usage Example

Here is a full integration example showing how to map standard, optional, JSON, skipped, and flattened fields.

```rust
use pg_query_mapper::PgQueryMapper;
use optional_field::Field;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};

// 1. Define sub-structs (e.g., for flattening)
#[derive(PgQueryMapper, Debug, PartialEq)]
#[pg_mapper(alias_prefix = "c_")]
pub struct Company {
    pub id: Uuid,
    pub company_no: String,
    pub name: Field<String>,        // Wrapped in `Field<T>` to make it optional
    pub description: Field<String>,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Profile {
    pub birth_date: Field<NaiveDate>,
    pub salary: Field<f64>,
}

#[derive(Default, Debug, PartialEq)]
pub struct Address {
    pub street: String,
    pub city: String,
}

// 2. Define the main target struct
#[derive(PgQueryMapper, Debug, PartialEq)]
#[pg_mapper(alias_prefix = "u_")]
pub struct User {
    pub id: Uuid,                                // Required column
    pub created_at: Field<DateTime<Utc>>,        // Optional column
    
    #[pg_mapper(rename = "user_name")]
    pub name: String,                            // Renamed column (ignores alias_prefix)

    #[pg_mapper(flatten)]
    pub company: Field<Company>,                 // Flattened sub-struct, optional via LEFT JOIN

    #[pg_mapper(json)]
    pub profile: Field<Profile>,                 // JSON deserialized column

    #[pg_mapper(skip)]
    pub addresses: Vec<Address>,                 // Skipped column (will instantiate with Default::default())
}

// 3. Map query results
async fn process_users(client: &tokio_postgres::Client) -> Result<(), Box<dyn std::error::Error>> {
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
    ";

    // Prepare statement
    let stmt = client.prepare(sql).await?;
    
    // Create the mapper (evaluates column indices once)
    let mapper = UserMapper::new(stmt.columns())?;

    // Query rows
    let rows = client.query(&stmt, &[]).await?;

    for row in rows {
        // Pure O(1) row mapping without string lookups
        let user: User = mapper.map(&row)?;
        println!("Mapped user: {:?}", user);
    }

    Ok(())
}
```

---

## 🎨 Struct & Field Attribute Reference

### Struct Attributes
* `#[pg_mapper(alias_prefix = "prefix_")]`: Automatically prepends `prefix_` to all struct field names when matching query column names.

### Field Attributes
* `#[pg_mapper(rename = "custom_name")]`: Explicitly matches a column named `"custom_name"` instead of the field name (ignores any struct-level `alias_prefix`).
* `#[pg_mapper(skip)]`: Completely ignores the field during mapping. The field's type must implement `Default`, and it will be populated with `Default::default()`.
* `#[pg_mapper(json)]`: Pulls the column as a raw JSON value (`serde_json::Value`) from `tokio-postgres` and deserializes it into the field's target type using `serde_json::from_value`.
* `#[pg_mapper(flatten)]`: Delegates mapping to another nested struct that derives `PgQueryMapper`.

---

## ⚠️ The LEFT JOIN Problem (Optional Flattened Structs)

When performing a `LEFT JOIN` on a table mapped via a flattened property wrapped in `Field<T>` (e.g. `pub company: Field<Company>`), the generated companion mapper automatically short-circuits. It checks if the primary/first column of the flattened struct is `NULL` (such as `c.id`). If it is, the property is automatically mapped as `Field::Present(None)` (missing) without executing parsing logic for the remaining columns, avoiding unnecessary errors.

## 📄 License

This project is licensed under the [MIT License](https://github.com/thotsaphon/pg-query-mapper/blob/main/LICENSE.md).
