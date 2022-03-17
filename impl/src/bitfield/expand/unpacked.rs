use quote::{format_ident, quote_spanned};
use syn::{Expr, Token};
use syn::__private::TokenStream2;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::{Add};

use crate::bitfield::BitfieldStruct;
use crate::bitfield::config::{Config, ReprKind};
use crate::bitfield::field_info::FieldInfo;

impl BitfieldStruct {
    /// Expands the given `#[bitfield]` struct into an actual bitfield definition.
    pub fn expand_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let check_filled = self.generate_check_for_filled(config);
        let struct_definition = self.generate_struct_unpacked(config);
        let constructor_definition = self.generate_constructor_unpacked(config);
        let specifier_impl = self.generate_specifier_impl(config);

        let byte_conversion_impls = self.generate_byte_conversion_impls_unpacked(config);
        let byte_update_impls = self.generate_byte_update_impls_unpacked(config);
        let getters_and_setters = self.generate_getters_and_setters_unpacked(config);
        let from_into_impl = self.generate_to_from_repr_unpacked(config);
        // let bytes_check = self.expand_optional_bytes_check(config);
        // let repr_impls_and_checks = self.expand_repr_from_impls_and_checks(config);

        quote_spanned!(span=>
            #struct_definition
            #check_filled
            #constructor_definition
            #byte_conversion_impls
            #byte_update_impls
            #getters_and_setters
            #specifier_impl
            #from_into_impl
            // #bytes_check
            // #repr_impls_and_checks
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

    /// Generates routines to allow conversion from and to bytes for the `#[bitfield]` struct.
    fn generate_byte_conversion_impls_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let ident = &self.item_struct.ident;
        let size = self.generate_target_or_actual_bitfield_size(config);
        let next_divisible_by_8 = Self::next_divisible_by_8(&size);
        let repr = self.get_repr_or_bits(config);
        let repr_type = repr.into_quote();

        let from_bytes = match config.filled_enabled() {
            true => {
                quote_spanned!(span=>
                    /// Converts the given bytes directly into the bitfield struct.
                    ///
                    /// Expects Little Endian byte order.
                    #[inline]
                    #[allow(clippy::identity_op)]
                    pub const fn from_le_bytes(bytes: [u8; #next_divisible_by_8 / 8usize]) -> Self {
                        let value = #repr_type::from_le_bytes(bytes);
                        value.into()
                    }
                )
            }
            false => {
                quote_spanned!(span=>
                    /// Converts the given bytes directly into the bitfield struct.
                    ///
                    /// # Errors
                    ///
                    /// If the given bytes contain bits at positions that are undefined for `Self`.
                    #[inline]
                    #[allow(clippy::identity_op)]
                    pub fn from_le_bytes(
                        bytes: [u8; #next_divisible_by_8 / 8usize]
                    ) -> ::core::result::Result<Self, ::modular_bitfield::error::OutOfBounds> {
                        if bytes[(#next_divisible_by_8 / 8usize) - 1] >= (0x01 << (8 - (#next_divisible_by_8 - #size))) {
                            return ::core::result::Result::Err(::modular_bitfield::error::OutOfBounds)
                        }

                        let value = #repr_type::from_le_bytes(bytes);

                        ::core::result::Result::Ok(value.into())
                    }
                )
            }
        };

        quote_spanned!(span=>
            impl #ident {
                /// Returns the underlying bits.
                ///
                /// # Layout
                ///
                /// Returns a little endian based layout.
                /// The returned byte array is laid out in the same way as described
                /// [here](https://docs.rs/modular-bitfield/#generated-structure).
                #[inline]
                #[allow(clippy::identity_op)]
                pub const fn to_le_bytes(self) -> [u8; #next_divisible_by_8 / 8usize] {
                    let value: #repr_type = self.into();
                    value.to_le_bytes()
                }

                #from_bytes
            }
        )
    }

    fn generate_byte_update_impls_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let ident = &self.item_struct.ident;
        let size = self.generate_target_or_actual_bitfield_size(config);
        let next_divisible_by_8 = Self::next_divisible_by_8(&size);
        let repr = self.get_repr_or_bits(config);
        let repr_type = repr.into_quote();

        quote_spanned!(span=>
            impl #ident {
                /// Updates the underlying byte.
                ///
                /// # Layout
                ///
                /// This is based on Little Endian indexing, aka, least significant byte is at index 0.
                #[inline]
                #[allow(clippy::identity_op)]
                pub fn update_byte_le(&mut self, byte: usize, value: u8) {
                    let int_val_self: #repr_type = (*self).into();
                    let mut value_le = int_val_self.to_le_bytes();

                    value_le[byte] = value;

                    let new_value = #repr_type::from_le_bytes(value_le);
                    *self = new_value.into();
                }

                /// Updates the underlying byte.
                ///
                /// # Layout
                ///
                /// This is based on Big Endian indexing, aka, most significant byte is at index 0.
                #[inline]
                #[allow(clippy::identity_op)]
                pub fn update_byte_be(&mut self, byte: usize, value: u8) {
                    let int_val_self: #repr_type = (*self).into();
                    let mut value_le = int_val_self.to_le_bytes();

                    value_le[(#next_divisible_by_8 / 8usize - 1 - byte)] = value;

                    let new_value = #repr_type::from_le_bytes(value_le);
                    *self = new_value.into();
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
            #[allow(dead_code)]
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

    fn generate_to_from_repr_unpacked(&self, config: &Config) -> TokenStream2 {
        let span = self.item_struct.span();
        let ident = &self.item_struct.ident;
        let mut offset = {
            let mut offset = Punctuated::<syn::Expr, Token![+]>::new();
            offset.push(syn::parse_quote! { 0usize });
            offset
        };

        let mut into_impls = Vec::new();
        let mut from_impls = Vec::new();

        let repr = self.get_repr_or_bits(config);
        let prim = repr.into_quote();

        let input_ident = quote_spanned! {span=> __bf_input_};
        let result_ident = quote_spanned! {span=> __bf_};

        for field in self.field_infos(config) {
            let ty = &field.field.ty;

            from_impls.push(self.expand_from_for_field(&mut offset, &field, &input_ident));
            into_impls.push(self.expand_into_for_field(&mut offset, &field, &prim, &input_ident, &result_ident));


            offset.push(syn::parse_quote! { <#ty as ::modular_bitfield::Specifier>::BITS });
        }

        quote_spanned!(span=>
                impl ::core::convert::From<#prim> for #ident
                {
                    #[inline]
                    #[allow(clippy::identity_op)]
                    fn from(#input_ident: #prim) -> Self {
                        Self {
                            #( #from_impls )*
                        }
                    }
                }

                impl ::core::convert::From<#ident> for #prim
                {
                    #[inline]
                    #[allow(clippy::identity_op)]
                    fn from(#input_ident: #ident) -> Self {
                        let mut #result_ident: #prim = 0;

                        #( #into_impls )*

                        #result_ident
                    }
                }
            )
    }

    fn expand_into_for_field(&self, offset: &mut Punctuated<Expr, Add>, info: &FieldInfo<'_>, primitive: &TokenStream2, input_ident: &TokenStream2, result_ident: &TokenStream2) -> Option<TokenStream2> {
        let FieldInfo {
            index: _, field,
            config, ..
        } = &info;
        let span = field.span();
        let ident = &field.ident;
        let ty = &field.ty;

        if config.skip_getters() {
            None
        } else {
            let result = quote_spanned! {span=>
                #result_ident |= (<#ty as ::modular_bitfield::Specifier>::into_bytes(#input_ident.#ident).unwrap() as #primitive) << (#offset);
            };

            Some(result)
        }
    }

    fn expand_from_for_field(&self, offset: &mut Punctuated<Expr, Add>, info: &FieldInfo<'_>, input_ident: &TokenStream2) -> Option<TokenStream2> {
        let FieldInfo {
            index: _, field,
            config, ..
        } = &info;
        let span = field.span();
        let ident = &field.ident;
        let ty = &field.ty;

        if config.skip_setters() {
            None
        } else {
            let result = quote_spanned! {span=>
                #ident: <#ty as ::modular_bitfield::Specifier>::from_bytes(((#input_ident >> (#offset)) & ((1 << (<#ty as ::modular_bitfield::Specifier>::BITS - #offset + 1)) - 1)) as <#ty as ::modular_bitfield::Specifier>::Bytes).unwrap(),
            };

            Some(result)
        }
    }

    fn get_repr_or_bits(&self, config: &Config) -> ReprKind {
        if let Some(rep) = config.repr.as_ref() {
            rep.value
        } else if let Some(bits) = config.bits.as_ref() {
            ReprKind::from_closest(bits.value as u8)
        } else {
            panic!("No repr or bits specified for {}", self.item_struct.ident);
        }
    }
}