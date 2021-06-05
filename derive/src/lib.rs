extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn;
use syn::{Data, Meta};

#[proc_macro_derive(ConfigPart)]
pub fn config_part_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    
    let data = match ast.data {
        Data::Struct(data) => data,
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    };
    
    let mut usage = Vec::new();
    for field in &data.fields {
        let field_name = field.ident.as_ref().unwrap();
        let mut doc = quote! { "" };
        
        for attr in &field.attrs {
            if let Ok(Meta::NameValue(meta)) = attr.parse_meta() {
                if let Some(ident) = meta.path.get_ident() {
                    if ident == "doc" {
                        doc = meta.lit.into_token_stream();
                    }
                }
            }
        }
        
        let ty = &field.ty;
        
        usage.push(quote! {{
            let path = format!("{}{}", path, stringify!(#field_name));
            let usage = <#ty>::usage_impl(&path, default.#field_name, #doc.trim());
            
            if usage.contains("\n") {
                groups.push(usage);
            } else {
                ret += &usage;
                ret += "\n";
            }
        }});
    }
    
    let mut usage_tokens = TokenStream::new();
    usage_tokens.extend(usage.into_iter());
    
    let name = &ast.ident;
    let gen = quote! {
        impl ConfigPart for #name {
            fn usage_impl(default: Self, path: &str, doc: &str) -> String {
                let mut ret = String::new();
                let mut groups = Vec::new();
                
                let path = if path.len() > 0 {
                    ret += &format!("\n{}:\n", doc);
                    format!("{}.", path)
                } else {
                    String::new()
                };
                
                #usage_tokens
                
                ret += &groups.join("");
                
                ret
            }
        }
    };
    
    gen.into()
}