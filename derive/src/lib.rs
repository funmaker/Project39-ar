extern crate proc_macro;

use std::iter::FromIterator;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{self, Data, Meta};

#[proc_macro_derive(FromArgs, attributes(arg_short, arg_rename))]
pub fn config_part_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    
    let data = match ast.data {
        Data::Struct(data) => data,
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    };
    
    let mut usage_impl = Vec::new();
    let mut prepare_opts = Vec::new();
    let mut apply_matches = Vec::new();
    
    for field in &data.fields {
        let field_name = field.ident.as_ref().unwrap();
        let mut doc = quote! { "" };
        let mut short = quote! { "" };
        let mut name = quote!( stringify!(#field_name) );
        
        for attr in &field.attrs {
            if let Ok(Meta::NameValue(meta)) = attr.parse_meta() {
                if let Some(ident) = meta.path.get_ident() {
                    match ident.to_string().as_str() {
                        "doc" => { doc = meta.lit.into_token_stream(); },
                        "arg_short" => { short = meta.lit.into_token_stream(); },
                        "arg_rename" => { name = meta.lit.into_token_stream(); },
                        _ => {},
                    }
                }
            }
        }
        
        usage_impl.push(quote! {{
            let mut path = path.to_string();
            if path.len() > 0 && #name.len() > 0 {
                path += ".";
            }
            path += #name;
            
            let usage = self.#field_name.usage_impl(#short, &path, #doc.trim());
            
            if usage.contains("\n") {
                groups.push(usage);
            } else {
                ret += &usage;
                ret += "\n";
            }
        }});
    
        prepare_opts.push(quote! {{
            let mut path = path.to_string();
            if path.len() > 0 && #name.len() > 0 {
                path += ".";
            }
            path += #name;
            
            self.#field_name.prepare_opts(opts, #short, &path, #doc.trim())?;
        }});
    
        apply_matches.push(quote! {{
            let mut path = path.to_string();
            if path.len() > 0 && #name.len() > 0 {
                path += ".";
            }
            path += #name;
            
            self.#field_name.apply_matches(matches, &path)?;
        }});
    }
    
    let usage_impl = TokenStream::from_iter(usage_impl.into_iter());
    let prepare_opts = TokenStream::from_iter(prepare_opts.into_iter());
    let apply_matches = TokenStream::from_iter(apply_matches.into_iter());
    
    let name = &ast.ident;
    let gen = quote! {
        
        impl FromArgs for #name {
            fn usage_impl(&self, _short: &str, path: &str, doc: &str) -> String {
                let mut ret = String::new();
                let mut groups = Vec::new();
                
                if path.len() > 0 {
                    ret += &format!("\n{}:\n", doc);
                }
                
                #usage_impl
                
                ret += &groups.join("");
                
                ret
            }
            
            fn prepare_opts(&mut self, opts: &mut Options, _short: &str, path: &str, doc: &str) -> Result<(), ArgsError> {
                #prepare_opts
                
                Ok(())
            }
				
            fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError> {
                #apply_matches
                
                Ok(())
            }
        }
    };
    
    gen.into()
}
