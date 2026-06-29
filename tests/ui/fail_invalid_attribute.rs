use pg_query_mapper::PgQueryMapper;

#[derive(PgQueryMapper)]
#[pg_mapper(unknown_magic_word)] // This is invalid at struct level
pub struct InvalidStruct {
    #[pg_mapper(another_unknown_magic)] // This is invalid at field level
    pub id: String,
}

fn main() {}
