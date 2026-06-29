use pg_query_mapper::PgQueryMapper;
use optional_field::Field;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;

#[derive(PgQueryMapper)]
#[pg_mapper(alias_prefix = "c_")]
pub struct Company {
    pub id: Uuid,
    pub company_no: String,
    pub name: Field<String>,
    pub description: Field<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Profile {
    pub birth_date: Field<NaiveDate>,
    pub salary: Field<Decimal>,
}

#[derive(Default)]
pub struct Address {
    pub user_name: String,
    pub street: String,
    pub city: String,
}

#[derive(PgQueryMapper)]
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

fn main() {
    // This file is compiled by trybuild to ensure the macro expansion type-checks successfully.
}
