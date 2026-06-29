pub use pg_query_mapper_derive::PgQueryMapper;

#[derive(thiserror::Error, Debug)]
pub enum MapperError {
    #[error("Missing column in result set: {0}")]
    MissingColumn(String),
}

pub trait PgQueryMapper: Sized {
    // The procedural macro generates a companion `<StructName>Mapper` struct.
    // We can define an associated type if we want, but it's not strictly necessary unless generic over mappers.
}
