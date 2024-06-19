#![feature(proc_macro_span)]
#![feature(proc_macro_quote)]
extern crate proc_macro2;

use darling::{ast::NestedMeta, Error, FromMeta};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn};

#[derive(FromMeta)]
struct InterruptHandlerMacroParam {
    int_vector: Option<u16>,
}

/// Generates a wrapper for static interrupt handlers.
#[proc_macro_attribute]
pub fn interrupt_handler(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let attr_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(Error::from(e).write_errors()),
    };

    let InterruptHandlerMacroParam { int_vector } =
        match InterruptHandlerMacroParam::from_list(&attr_args) {
            Ok(p) => p,
            Err(_) => InterruptHandlerMacroParam { int_vector: None },
        };

    let ItemFn {
        attrs: _,
        vis: _,
        sig,
        block,
    } = input_fn;

    if sig.inputs.len() != 1 {
        panic!(
            "invalid number of arguments for handler {}",
            sig.ident.to_string()
        );
    }

    let fn_body = &block.stmts;

    let wrapped_fn_name = format!("__int_handler_wrapped_{}", sig.ident.to_string());
    let wrapped_fn_ident = Ident::new(wrapped_fn_name.as_str(), Span::mixed_site());
    let wrapped_int_handler = quote! {
        #[no_mangle]
        #[link_section = ".int"]
        pub extern "C" fn #wrapped_fn_ident (frame: crate::fzboot::irq::InterruptStackFrame) {
            #(#fn_body)*
        }
    };

    #[cfg(not(feature = "x86_64"))]
    // Define wrapper assembly
    let wrapper = format!(
        "pushad
                call _int_entry
                call {}
                call _pic_eoi
                popad
                iretd",
        wrapped_fn_name
    );

    #[cfg(feature = "x86_64")]
    // Define wrapper assembly
    // TODO: save registers ?
    let wrapper = format!(
        "
        push r15
        push r14
        push r13
        push r12
        push r11
        push r10
        push r9
        push r8
        push rbp
        push rdi
        push rsi
        push rdx
        push rcx
        push rbx
        push rax
        mov rbp, rsp
        mov rax, [rbp + 0x90]
        push rax
        mov rax, [rbp + 0x88]
        push rax
        mov rax, [rbp + 0x80]
        push rax
        mov rax, [rbp + 0x78]
        push rax
        mov rax, [rbp + 0x70]
        push rax
        mov rdi, rsp
        call {}
        call _pic_eoi
        add rsp, 0x28
        pop rax
        pop rbx
        pop rcx
        pop rdx
        pop rsi
        pop rdi
        pop rbp
        pop r8
        pop r9
        pop r10
        pop r11
        pop r12
        pop r13
        pop r14
        pop r15
        iretq",
        wrapped_fn_name
    );

    let wrapper_ident = Ident::new(&sig.ident.to_string(), Span::mixed_site());

    let wrapper = quote! {
        #[link_section = ".int"]
        #[naked]
        pub fn #wrapper_ident () {
            unsafe {
                core::arch::asm!(
                    #wrapper
                , options(noreturn))
            }
        }
    };

    let stream = quote! {
        #wrapped_int_handler
        #wrapper
    };

    stream.into()
}

/// Generates interrupt handler entry points for dynamically registered interrupt handlers.
#[proc_macro]
pub fn generate_runtime_handlers_wrapper(_item: TokenStream) -> TokenStream {
    let mut handlers = Vec::new();
    let mut mappings = Vec::new();

    for i in 0u8..=255 {
        let wrapper_name = Ident::new(
            &format!("__runtime_handler_wrapper_{}", i),
            Span::mixed_site(),
        );

        let wrapped_name = Ident::new(&format!("__runtime_handler_{}", i), Span::mixed_site());

        #[cfg(not(feature = "x86_64"))]
        let wrapper = format!(
            "pushad
            call _int_entry
            call {}
            call _pic_eoi
            popad
            iretd",
            wrapped_name
        );

        #[cfg(feature = "x86_64")]
        let wrapper = format!(
            "
            push r15
            push r14
            push r13
            push r12
            push r11
            push r10
            push r9
            push r8
            push rbp
            push rdi
            push rsi
            push rdx
            push rcx
            push rbx
            push rax
            mov rbp, rsp
            mov rax, [rbp + 0x90]
            push rax
            mov rax, [rbp + 0x88]
            push rax
            mov rax, [rbp + 0x80]
            push rax
            mov rax, [rbp + 0x78]
            push rax
            mov rax, [rbp + 0x70]
            push rax
            mov rdi, rsp
            call {}
            call _pic_eoi
            add rsp, 0x28
            pop rax
            pop rbx
            pop rcx
            pop rdx
            pop rsi
            pop rdi
            pop rbp
            pop r8
            pop r9
            pop r10
            pop r11
            pop r12
            pop r13
            pop r14
            pop r15
            iretq",
            wrapped_name
        );

        let handler = quote! {
            #[inline(always)]
            #[no_mangle]
            pub fn #wrapped_name (frame: crate::fzboot::irq::InterruptStackFrame) {
                crate::fzboot::irq::handlers::_runtime_int_entry(InterruptVector::from(#i));
            }

            #[link_section = ".int"]
            #[naked]
            pub fn #wrapper_name () {
                unsafe {
                    core::arch::asm!(
                        #wrapper
                    , options(noreturn))
                }
            }
        };

        let mapping = quote! {
            #i, #wrapper_name as fn()
        };

        mappings.push(mapping);
        handlers.push(handler);
    }

    let stream = quote! {
        lazy_static::lazy_static! {
            pub static ref __RUNTIME_HANDLER_WRAPPER_MAPPINGS: hashbrown::HashMap<u8, fn()> = {
                let mut map = hashbrown::HashMap::new();
                #(map.insert(#mappings);)*
                map
            };
        }
        #(#handlers)*
    };

    stream.into()
}
