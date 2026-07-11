use std::{collections::HashSet, sync::LazyLock};

use proc_macro::TokenStream;
use quote::quote;
use syn::{Ident, parse_macro_input};

static LUCIDE_ICONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("../../qingqi-ui/assets/lucide-icons.txt")
        .lines()
        .filter(|line| !line.is_empty())
        .collect()
});

/// Validate a Lucide icon identifier and expand it to its embedded asset path.
#[proc_macro]
pub fn lucide_path(input: TokenStream) -> TokenStream {
    let ident = parse_macro_input!(input as Ident);
    let name = ident.to_string().trim_start_matches("r#").replace('_', "-");

    if !LUCIDE_ICONS.contains(name.as_str()) {
        let message = format!("unknown Lucide icon `{name}`");
        return syn::Error::new(ident.span(), message)
            .to_compile_error()
            .into();
    }

    let path = format!("lucide/{name}.svg");
    quote!(#path).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icon_list_contains_representative_names() {
        assert!(LUCIDE_ICONS.contains("search"));
        assert!(LUCIDE_ICONS.contains("settings-2"));
        assert!(LUCIDE_ICONS.contains("wrap-text"));
        assert!(LUCIDE_ICONS.len() > 1_000);
    }
}
