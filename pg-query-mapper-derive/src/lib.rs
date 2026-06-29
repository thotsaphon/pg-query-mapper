use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, GenericArgument, PathArguments, Type};

#[proc_macro_derive(PgQueryMapper, attributes(pg_mapper))]
pub fn pg_query_mapper_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_ident = input.ident;
    let mapper_ident = format_ident!("{}Mapper", struct_ident);

    let mut alias_prefix = String::new();

    // Parse struct attributes
    for attr in &input.attrs {
        if attr.path().is_ident("pg_mapper") {
            let res = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("alias_prefix") {
                    let value = meta.value()?;
                    let s: syn::LitStr = value.parse()?;
                    alias_prefix = s.value();
                    Ok(())
                } else {
                    Err(meta.error("unsupported struct attribute for pg_mapper"))
                }
            });
            if let Err(err) = res {
                return TokenStream::from(err.into_compile_error());
            }
        }
    }

    let fields = match input.data {
        Data::Struct(ref data) => match data.fields {
            syn::Fields::Named(ref fields) => &fields.named,
            _ => panic!("PgQueryMapper only supports structs with named fields"),
        },
        _ => panic!("PgQueryMapper only supports structs"),
    };

    let mut mapper_fields = Vec::new();
    let mut new_vars = Vec::new();
    let mut new_match_arms = Vec::new();
    let mut new_struct_fields = Vec::new();
    let mut map_fields = Vec::new();

    let mut first_required_idx_ident = None;
    let mut first_required_ty = None;

    for field in fields {
        let field_ident = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        let idx_ident = format_ident!("{}_idx", field_ident);

        let mut rename = None;
        let mut is_json = false;
        let mut is_flatten = false;
        let mut is_skip = false;

        for attr in &field.attrs {
            if attr.path().is_ident("pg_mapper") {
                let res = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        let value = meta.value()?;
                        let s: syn::LitStr = value.parse()?;
                        rename = Some(s.value());
                        Ok(())
                    } else if meta.path.is_ident("json") {
                        is_json = true;
                        Ok(())
                    } else if meta.path.is_ident("flatten") {
                        is_flatten = true;
                        Ok(())
                    } else if meta.path.is_ident("skip") {
                        is_skip = true;
                        Ok(())
                    } else {
                        Err(meta.error("unsupported field attribute for pg_mapper"))
                    }
                });
                if let Err(err) = res {
                    return TokenStream::from(err.into_compile_error());
                }
            }
        }

        if is_skip {
            map_fields.push(quote! {
                #field_ident: Default::default()
            });
            continue;
        }

        let is_opt = is_field_wrapper(ty);

        if is_flatten {
            let inner_ty = if is_opt {
                extract_inner_type(ty).unwrap_or(ty)
            } else {
                ty
            };

            let nested_mapper_ident = format_ident!("{}Mapper", get_type_ident(inner_ty).unwrap());
            let mapper_field_ident = format_ident!("{}_mapper", field_ident);

            mapper_fields.push(quote! {
                #mapper_field_ident: #nested_mapper_ident
            });

            new_struct_fields.push(quote! {
                #mapper_field_ident: #nested_mapper_ident::new(columns)?
            });

            if is_opt {
                map_fields.push(quote! {
                    #field_ident: self.#mapper_field_ident.map_optional(row)?
                });
            } else {
                map_fields.push(quote! {
                    #field_ident: self.#mapper_field_ident.map(row)?
                });
            }
            continue;
        }

        let col_name = rename
            .clone()
            .unwrap_or_else(|| format!("{}{}", alias_prefix, field_ident));

        if !is_opt && !is_json {
            // Required field
            mapper_fields.push(quote! { #idx_ident: usize });
            new_vars.push(quote! { let mut #idx_ident = None; });
            new_match_arms.push(quote! { #col_name => #idx_ident = Some(idx), });
            new_struct_fields.push(quote! {
                #idx_ident: #idx_ident.ok_or_else(|| pg_query_mapper::MapperError::MissingColumn(#col_name.into()))?
            });

            map_fields.push(quote! {
                #field_ident: row.try_get(self.#idx_ident)?
            });

            if first_required_idx_ident.is_none() {
                first_required_idx_ident = Some(idx_ident.clone());
                first_required_ty = Some(ty.clone());
            }
        } else {
            // Optional or JSON field
            mapper_fields.push(quote! { #idx_ident: Option<usize> });
            new_vars.push(quote! { let mut #idx_ident = None; });
            new_match_arms.push(quote! { #col_name => #idx_ident = Some(idx), });
            new_struct_fields.push(quote! { #idx_ident });

            if is_json {
                // If it's a Field<T>, we need to wrap the deserialized object in Field::Present
                if is_opt {
                    map_fields.push(quote! {
                        #field_ident: match self.#idx_ident {
                            Some(idx) => {
                                let raw: Option<serde_json::Value> = row.try_get(idx)?;
                                match raw {
                                    Some(val) => optional_field::Field::Present(Some(
                                        serde_json::from_value(val).unwrap_or_else(|e| panic!("JSON parse error: {}", e))
                                    )),
                                    None => optional_field::Field::Present(None),
                                }
                            },
                            None => optional_field::Field::Missing,
                        }
                    });
                } else {
                    map_fields.push(quote! {
                        #field_ident: match self.#idx_ident {
                            Some(idx) => {
                                let raw: Option<serde_json::Value> = row.try_get(idx)?;
                                match raw {
                                    Some(val) => serde_json::from_value(val).unwrap_or_else(|e| panic!("JSON parse error: {}", e)),
                                    None => panic!("Missing required JSON column data for {}", #col_name),
                                }
                            },
                            None => panic!("Missing required JSON column index for {}", #col_name),
                        }
                    });
                }
            } else {
                map_fields.push(quote! {
                    #field_ident: match self.#idx_ident {
                        Some(idx) => optional_field::Field::Present(row.try_get(idx)?),
                        None => optional_field::Field::Missing,
                    }
                });
            }
        }
    }

    let map_optional_method = if let (Some(first_idx), Some(first_ty)) =
        (first_required_idx_ident, first_required_ty)
    {
        quote! {
            pub fn map_optional(&self, row: &tokio_postgres::Row) -> Result<optional_field::Field<#struct_ident>, tokio_postgres::Error> {
                let first_val: Option<#first_ty> = row.try_get(self.#first_idx)?;
                if first_val.is_none() {
                    return Ok(optional_field::Field::Present(None));
                }

                let mapped = self.map(row)?;
                Ok(optional_field::Field::Present(Some(mapped)))
            }
        }
    } else {
        quote! {
            pub fn map_optional(&self, row: &tokio_postgres::Row) -> Result<optional_field::Field<#struct_ident>, tokio_postgres::Error> {
                let mapped = self.map(row)?;
                Ok(optional_field::Field::Present(Some(mapped)))
            }
        }
    };

    let expanded = quote! {
        pub struct #mapper_ident {
            #(#mapper_fields),*
        }

        impl #mapper_ident {
            pub fn new(columns: &[tokio_postgres::Column]) -> Result<Self, pg_query_mapper::MapperError> {
                #(#new_vars)*

                for (idx, column) in columns.iter().enumerate() {
                    match column.name() {
                        #(#new_match_arms)*
                        _ => {}
                    }
                }

                Ok(Self {
                    #(#new_struct_fields),*
                })
            }

            pub fn map(&self, row: &tokio_postgres::Row) -> Result<#struct_ident, tokio_postgres::Error> {
                Ok(#struct_ident {
                    #(#map_fields),*
                })
            }

            #map_optional_method
        }
    };

    TokenStream::from(expanded)
}

fn is_field_wrapper(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Field";
        }
    }
    false
}

fn extract_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Field" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

fn get_type_ident(ty: &Type) -> Option<&syn::Ident> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return Some(&segment.ident);
        }
    }
    None
}
