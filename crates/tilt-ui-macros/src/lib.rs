//! Rust-side component and template handler registration.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, FnArg, GenericArgument, Item, ItemFn, LitStr, PathArguments, Token, Type,
    UseTree, parse::Parser, parse_macro_input, punctuated::Punctuated,
};

/// Marks a component logic item. Its template and styles are discovered by the build script.
#[proc_macro_attribute]
pub fn ui_component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Compatibility marker for Rust component registries generated at build time.
#[proc_macro_attribute]
pub fn ui_registry(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Registers a function returning a `Routes` table for the TiltUI router.
#[proc_macro_attribute]
pub fn ui_routes(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return quote!(compile_error!("ui_routes takes no arguments");).into();
    }
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    quote! {
        #function

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::RoutesRegistration { build: #name }
        }
    }
    .into()
}

/// Registers a normal Bevy system in the `Startup` schedule.
#[proc_macro_attribute]
pub fn component_init(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return quote!(compile_error!("component_init takes no arguments");).into();
    }
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    let register = format_ident!("__tilt_ui_register_init_{}", name);
    quote! {
        #function

        #[doc(hidden)]
        fn #register(app: &mut ::tilt_ui::bevy::app::App) {
            use ::tilt_ui::bevy::app::Startup;
            app.add_systems(Startup, #name);
        }

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::ComponentInitRegistration { register: #register }
        }
    }
    .into()
}

/// Registers an ordinary Bevy system in the `Update` schedule.
#[proc_macro_attribute]
pub fn component_update(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return quote!(compile_error!("component_update takes no arguments");).into();
    }
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    let register = format_ident!("__tilt_ui_register_update_{}", name);
    quote! {
        #function

        #[doc(hidden)]
        fn #register(app: &mut ::tilt_ui::bevy::app::App) {
            use ::tilt_ui::bevy::app::Update;
            app.add_systems(Update, #name);
        }

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::ComponentUpdateRegistration { register: #register }
        }
    }
    .into()
}

/// Registers an `In<HtmlEvent>` Bevy system for an HTML handler name.
#[proc_macro_attribute]
pub fn html_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let handler = parse_macro_input!(attr as LitStr);
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    let register = format_ident!("__tilt_ui_register_html_fn_{}", name);
    let registration = match input_type(&function) {
        Some(event_type) if !is_html_event(&event_type) => quote! {
            let typed = world.register_system(#name);
            world.register_system(move |In(event): In<::tilt_ui::HtmlEvent>, world: &mut ::tilt_ui::bevy::ecs::world::World| {
                if let Ok(value) = <#event_type as TryFrom<::tilt_ui::HtmlEvent>>::try_from(event) {
                    let _ = world.run_system_with(typed, value);
                }
            })
        },
        _ => quote!(world.register_system(#name)),
    };
    quote! {
        #function

        #[doc(hidden)]
        fn #register(world: &mut ::tilt_ui::bevy::ecs::world::World)
            -> ::tilt_ui::bevy::ecs::system::SystemId<
                ::tilt_ui::bevy::prelude::In<::tilt_ui::HtmlEvent>, ()
            >
        {
            use ::tilt_ui::bevy::ecs::system::In;
            #registration
        }

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::HtmlHandlerRegistration {
                name: #handler,
                register: #register,
            }
        }
    }
    .into()
}

/// Exposes one pure value method to expressions in a named component template.
///
/// The function signature is `fn(&serde_json::Value, &[serde_json::Value])
/// -> Option<serde_json::Value>`. Its first argument is the resolved receiver.
#[proc_macro_attribute]
pub fn html_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attributes = match Punctuated::<LitStr, Token![,]>::parse_terminated.parse(attr) {
        Ok(attributes) if attributes.len() == 2 => attributes,
        _ => {
            return quote!(compile_error!("html_method expects a component name and a receiver.method path");).into();
        }
    };
    let mut attributes = attributes.into_iter();
    let component = attributes.next().unwrap();
    let path = attributes.next().unwrap();
    if component.value().is_empty() || !valid_method_path(&path.value()) {
        return quote!(compile_error!("html_method requires a nonempty component name and a dotted identifier path such as user.full_name");).into();
    }
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    quote! {
        #function

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::HtmlMethodRegistration {
                component: #component,
                path: #path,
                evaluate: #name,
            }
        }
    }
    .into()
}

fn valid_method_path(path: &str) -> bool {
    let segments = path.split('.').collect::<Vec<_>>();
    segments.len() >= 2
        && segments.iter().all(|segment| {
            segment.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
}

fn input_type(function: &ItemFn) -> Option<Type> {
    let FnArg::Typed(argument) = function.sig.inputs.first()? else {
        return None;
    };
    let Type::Path(ty) = argument.ty.as_ref() else {
        return None;
    };
    let segment = ty.path.segments.last()?;
    if segment.ident != "In" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let GenericArgument::Type(event) = arguments.args.first()? else {
        return None;
    };
    Some(event.clone())
}

fn is_html_event(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "HtmlEvent"))
}

/// Registers a default, serializable type in TiltUI's typed binding store.
#[proc_macro_derive(UiStore, attributes(ui_store))]
pub fn derive_ui_store(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    if matches!(input.data, Data::Union(_)) || !input.generics.params.is_empty() {
        return quote!(compile_error!("UiStore supports only non-generic structs and enums");)
            .into();
    }
    let mutable = match input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("ui_store"))
        .map(|attribute| attribute.parse_args::<syn::Ident>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(attributes) if attributes.len() <= 1 => attributes
            .first()
            .is_some_and(|attribute| attribute == "mutable"),
        _ => {
            return quote!(compile_error!("ui_store accepts only #[ui_store(mutable)] once");)
                .into();
        }
    };
    if input
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("ui_store"))
        && !mutable
    {
        return quote!(compile_error!("ui_store accepts only #[ui_store(mutable)]");).into();
    }
    let name = input.ident;
    let register = format_ident!("__tilt_ui_register_store_{}", name);
    let register_call = if mutable {
        quote!(store.register_mutable::<#name>();)
    } else {
        quote!(store.register::<#name>();)
    };
    quote! {
        impl ::tilt_ui::UiStore for #name {
            const STORE_KEY: &'static str = stringify!(#name);
            const STORE_PATH: &'static str = concat!(module_path!(), "::", stringify!(#name));
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #register(store: &mut ::tilt_ui::UiBindingStore) {
            #register_call
        }

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::UiStoreRegistration { register: #register }
        }
    }
    .into()
}

/// Exposes a Bevy resource's serializable fields to template bindings.
#[proc_macro_attribute]
pub fn html_shared(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shared(attr, item)
}

/// Exposes a Bevy resource with its lowercase type-name alias.
#[proc_macro_attribute]
pub fn html_use(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_shared(attr, item)
}

fn expand_shared(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return quote!(compile_error!("html_shared and html_use take no arguments");).into();
    }
    let parsed = parse_macro_input!(item as Item);
    let name = match &parsed {
        Item::Struct(item) => item.ident.clone(),
        Item::Enum(item) => item.ident.clone(),
        Item::Use(item) => match last_use_name(&item.tree) {
            Some(name) => name,
            None => return quote!(compile_error!("html_shared supports only one imported type");).into(),
        },
        _ => return quote!(compile_error!("html_shared supports structs, enums and single type imports");).into(),
    };
    let register = format_ident!("__tilt_ui_capture_shared_{}", name);
    let changed = format_ident!("__tilt_ui_shared_changed_{}", name);
    let present = format_ident!("__tilt_ui_shared_present_{}", name);
    let type_name = name.to_string();
    let alias = lower_first(&type_name);
    quote! {
        #parsed

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #register(world: &::tilt_ui::bevy::ecs::world::World)
            -> Option<::tilt_ui::serde_json::Value>
        {
            world.get_resource::<#name>()
                .and_then(|value| ::tilt_ui::serde_json::to_value(value).ok())
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #changed(world: &::tilt_ui::bevy::ecs::world::World) -> bool {
            use ::tilt_ui::bevy::ecs::change_detection::DetectChanges;
            world.get_resource_ref::<#name>()
                .is_some_and(|value| value.is_changed())
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #present(world: &::tilt_ui::bevy::ecs::world::World) -> bool {
            world.contains_resource::<#name>()
        }

        ::tilt_ui::inventory::submit! {
            ::tilt_ui::SharedValueRegistration {
                key: #type_name,
                alias: #alias,
                snapshot: #register,
                changed: #changed,
                present: #present,
            }
        }
    }
    .into()
}

fn last_use_name(tree: &UseTree) -> Option<syn::Ident> {
    match tree {
        UseTree::Path(path) => last_use_name(&path.tree),
        UseTree::Name(name) => Some(name.ident.clone()),
        UseTree::Rename(rename) => Some(rename.rename.clone()),
        _ => None,
    }
}

fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(char::to_lowercase)
        .into_iter()
        .flatten()
        .collect::<String>()
        + chars.as_str()
}
