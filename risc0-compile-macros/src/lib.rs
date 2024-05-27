use quote::quote;
use risc0_zkvm::compute_image_id;
use std::{env, path::PathBuf};
use syn::{Error, LitStr};
use toml::Value;

#[proc_macro]
pub fn compiled_guest(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as GuestInput);
    generate_consts(input).into()
}

struct GuestInput {
    /// Resulting mod name.
    mod_name: syn::Ident,
    /// Path to the guest crate `Cargo.toml`.
    _manifest_path: PathBuf,
    /// Crate name of manifest path.
    crate_name: String,
}

impl syn::parse::Parse for GuestInput {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        input.parse::<syn::Token![mod]>()?;
        let mod_name = input.parse::<syn::Ident>()?;
        input.parse::<syn::Token![,]>()?;

        let lit = input.parse::<LitStr>()?;
        let span = lit.span();

        let mut path = PathBuf::from(lit.value());
        if path.is_relative() {
            let dir = std::env::var_os("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .ok_or_else(|| Error::new(span, "failed to get manifest dir"))?;
            path = dir.join(path);
        }
        path = dunce::canonicalize(&path)
            .map_err(|e| Error::new(span, format!("failed to canonicalize path {path:?}: {e}")))?;

        let manifest_path = path.join("Cargo.toml");
        let cargo_toml_content =
            std::fs::read_to_string(&manifest_path).expect("Failed to read Cargo.toml");
        let cargo_toml: Value = cargo_toml_content
            .parse()
            .expect("Failed to parse Cargo.toml");
        let crate_name = cargo_toml
            .get("package")
            .and_then(|package| package.get("name"))
            .expect("Failed to find crate name in Cargo.toml");

        Ok(GuestInput {
            mod_name,
            _manifest_path: manifest_path,
            crate_name: crate_name.as_str().unwrap().to_string(),
        })
    }
}

fn generate_consts(input: GuestInput) -> proc_macro2::TokenStream {
    let GuestInput {
        mod_name,
        _manifest_path: _,
        crate_name,
    } = input;

    let bin_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("no manifest env"))
        .parent()
        .unwrap()
        .join("target")
        .join("riscv32im-risc0-zkvm-elf")
        .join("release")
        .join(crate_name);
    let bin_path_str = bin_path.to_str().unwrap();
    let bin = std::fs::read(&bin_path).unwrap();
    let guest_id: [u32; 8] = compute_image_id(&bin).unwrap().into();
    quote! {
        pub mod #mod_name {
            pub const ELF: &[u8] = include_bytes!(#bin_path_str);
            pub const ID: [u32; 8] = [#(#guest_id,)*];
        }
    }
}
