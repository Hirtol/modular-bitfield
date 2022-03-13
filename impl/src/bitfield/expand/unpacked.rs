use quote::{format_ident, quote, quote_spanned};
use syn::{Token, Type};
use syn::__private::TokenStream2;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::bitfield::BitfieldStruct;
use crate::bitfield::config::Config;
use crate::bitfield::field_info::FieldInfo;

impl BitfieldStruct {
    /// Expands the given `#[bitfield]` struct into an actual bitfield definition.
    pub fn expand_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let check_filled = self.generate_check_for_filled(config);
        let struct_definition = self.generate_struct_unpacked(config);
        let constructor_definition = self.generate_constructor_unpacked(config);
        let specifier_impl = self.generate_specifier_impl(config);

        // let byte_conversion_impls = self.expand_byte_conversion_impls(config);
        // let byte_update_impls = self.expand_byte_update_impls(config);
        let getters_and_setters = self.generate_getters_and_setters_unpacked(config);
        // let bytes_check = self.expand_optional_bytes_check(config);
        // let repr_impls_and_checks = self.expand_repr_from_impls_and_checks(config);
        // let debug_impl = self.generate_debug_impl(config);

        quote_spanned!(span=>
            #struct_definition
            #check_filled
            #constructor_definition
            // #byte_conversion_impls
            // #byte_update_impls
            #getters_and_setters
            #specifier_impl
            // #bytes_check
            // #repr_impls_and_checks
            // #debug_impl
        )
    }

    /// Generates the constructor for the bitfield that initializes all bytes to zero.
    fn generate_constructor_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let ident = &self.item_struct.ident;

        let fields = self.field_infos(config).filter(|f| !f.config.skip_all());
        let field_names = fields.map(|f| &f.field.ident);
        let fields = self.field_infos(config).filter(|f| !f.config.skip_all());
        let field_types = fields.map(|f| &f.field.ty);

        quote_spanned!(span=>
            impl #ident
            {
                /// Returns an instance with zero initialized data.
                #[allow(clippy::identity_op)]
                pub fn new() -> Self {
                    Self {
                        #( #field_names: <#field_types as ::modular_bitfield::Specifier>::from_bytes(0).expect("Failed to initialise field"), )*
                    }
                }
            }
        )
    }

    /// Generates the actual item struct definition for the `#[bitfield]`.
    ///
    /// Internally it only contains a byte array equal to the minimum required
    /// amount of bytes to compactly store the information of all its bit fields.
    fn generate_struct_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let attrs = &config.retained_attributes;
        let vis = &self.item_struct.vis;
        let ident = &self.item_struct.ident;

        let bits_checks = self
            .field_infos(config)
            .map(|field_info| self.expand_bits_checks_for_field(field_info));

        let fields_true = self.field_infos(config)
            .filter(|f_info| !f_info.config.skip_all())
            .map(|field_info| {
                self.expand_field_unpacked(field_info)
            });

        quote_spanned!(span=>
            #( #attrs )*
            #[allow(clippy::identity_op)]
            #vis struct #ident
            {
                #( #fields_true )*
            }

            const _: () = {
                #( #bits_checks )*
            };
        )
    }

    fn expand_field_unpacked(
        &self,
        info: FieldInfo<'_>,
    ) -> Option<TokenStream2> {
        let FieldInfo {
            index: _, field, ..
        } = &info;

        let span = field.span();
        let ident = &field.ident;
        let ty = &field.ty;

        let field_token = quote_spanned!(span=>
            #ident: <#ty as ::modular_bitfield::Specifier>::InOut,
        );

        Some(field_token)
    }

    fn generate_getters_and_setters_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let ident = &self.item_struct.ident;

        let setters_and_getters = self.field_infos(config).map(|field_info| {
            self.expand_getters_and_setters_for_field_unpacked(field_info)
        });

        quote_spanned!(span=>
            impl #ident {
                #( #setters_and_getters )*
            }
        )
    }

    fn expand_getters_and_setters_for_field_unpacked(
        &self,
        info: FieldInfo<'_>,
    ) -> Option<TokenStream2> {
        let FieldInfo {
            index: _, field, ..
        } = &info;
        let span = field.span();

        let getters = self.expand_getters_for_field_unpacked(&info);
        let setters = self.expand_setters_for_field_unpacked(&info);

        let getters_and_setters = quote_spanned!(span=>
            #getters
            #setters
        );

        Some(getters_and_setters)
    }

    fn expand_getters_for_field_unpacked(
        &self,
        info: &FieldInfo<'_>,
    ) -> Option<TokenStream2> {
        let FieldInfo {
            index: _,
            field,
            config,
        } = &info;

        if config.skip_getters() {
            return None;
        }

        let span = field.span();
        let ident = info.ident_frag();
        let name = info.name();

        let retained_attrs = &config.retained_attrs;
        let get_ident = field
            .ident
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format_ident!("get_{}", ident));

        let ty = &field.ty;
        let vis = &field.vis;
        let real_ident = &field.ident;

        let getter_docs = format!("Returns the value of {}.\n", name);

        let getters = quote_spanned!(span=>
            #[doc = #getter_docs]
            #[inline]
            #( #retained_attrs )*
            #vis const fn #get_ident(&self) -> <#ty as ::modular_bitfield::Specifier>::InOut {
                self.#real_ident
            }
        );
        Some(getters)
    }

    fn expand_setters_for_field_unpacked(
        &self,
        info: &FieldInfo<'_>,
    ) -> Option<TokenStream2> {
        let FieldInfo {
            index: _,
            field,
            config,
        } = &info;

        if config.skip_setters() {
            return None;
        }

        let span = field.span();
        let retained_attrs = &config.retained_attrs;

        let ident = info.ident_frag();
        let name = info.name();
        let ty = &field.ty;
        let vis = &field.vis;
        let real_ident = &field.ident;

        let set_ident = format_ident!("set_{}", ident);
        let with_ident = format_ident!("with_{}", ident);
        let setter_docs = format!(
            "Sets the value of {} to the given value.\n\n\
             #Panics\n\n\
             If the given value is out of bounds for {}.\n",
            name, name,
        );
        let with_docs = format!(
            "Returns a copy of the bitfield with the value of {} \
             set to the given value.\n\n\
             #Panics\n\n\
             If the given value is out of bounds for {}.\n",
            name, name,
        );

        let setters = quote_spanned!(span=>
            #[doc = #with_docs]
            #[inline]
            #[allow(dead_code)]
            #( #retained_attrs )*
            #vis fn #with_ident(
                mut self,
                new_val: <#ty as ::modular_bitfield::Specifier>::InOut
            ) -> Self {
                self.#set_ident(new_val);
                self
            }

            #[doc = #setter_docs]
            #[inline]
            #[allow(dead_code)]
            #( #retained_attrs )*
            #vis fn #set_ident(&mut self, new_val: <#ty as ::modular_bitfield::Specifier>::InOut) {
                self.#real_ident = new_val;
            }
        );
        Some(setters)
    }
}