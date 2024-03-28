extern crate proc_macro;

use std::iter::FromIterator;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Data, Meta};

#[proc_macro_derive(FromArgs, attributes(arg_short, arg_rename, arg_skip))]
pub fn from_args_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
            if attr.path
                   .get_ident()
                   .map_or(false, |i| i.to_string() == "arg_skip") {
                continue;
            }
            
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


#[proc_macro_derive(ComponentBase, attributes(inner))]
pub fn component_base_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let mut inner = None;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    
    match ast.data {
        Data::Struct(data) => {
            for field in &data.fields {
                for attr in &field.attrs {
                    if attr.path
                           .get_ident()
                           .map_or(false, |i| i.to_string() == "inner") {
                        let field_name = field.ident.clone().unwrap();
                
                        if let Some(_) = inner.replace(field_name) {
                            panic!("Duplicate #[inner] attribute.");
                        }
                    }
                }
            }
        },
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    };
    
    let inner = inner.expect("Missing #[inner] attribute.");
    
    let gen = quote! {
        impl #impl_generics ComponentBase for #name #ty_generics #where_clause {
            fn inner(&self) -> &ComponentInner {
                &self.#inner
            }
            
            fn inner_mut(&mut self) -> &mut ComponentInner {
                &mut self.#inner
            }
            
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
            
            fn name(&self) -> &'static str {
                stringify!(#name)
            }
        }
    };
    
    gen.into()
}
