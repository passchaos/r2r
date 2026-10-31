#[cfg(not(feature = "doc-only"))]
use {
    quote::{format_ident, quote},
    rayon::prelude::*,
    std::fmt,
    std::fs::File,
    std::io::{self, prelude::*, BufWriter},
};

use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "doc-only")]
mod filenames {
    pub const LIST_FILENAME: &str = "files.txt";
    pub const CFG_FILENAME: &str = "_r2r_generated_cfg.txt";
}

#[cfg(not(feature = "doc-only"))]
mod filenames {
    pub const LIST_FILENAME: &str = "files.txt";
    pub const CFG_FILENAME: &str = "_r2r_generated_cfg.txt";
    pub const MSGS_FILENAME: &str = "_r2r_generated_msgs.rs";
    pub const UNTYPED_FILENAME: &str = "_r2r_generated_untyped_helper.rs";
    pub const UNTYPED_SERVICE_FILENAME: &str = "_r2r_generated_service_helper.rs";
    pub const UNTYPED_ACTION_FILENAME: &str = "_r2r_generated_action_helper.rs";
    pub const GENERATED_FILES: &[&str] = &[
        MSGS_FILENAME,
        UNTYPED_FILENAME,
        UNTYPED_SERVICE_FILENAME,
        UNTYPED_ACTION_FILENAME,
        CFG_FILENAME,
    ];
}

use filenames::*;

fn main() {
    // Cargo rebuilds the build-dependency executable when the generator changes, but does not
    // reliably rerun this package's build script solely for that dependency-source change.
    println!("cargo:rerun-if-changed=../r2r_msg_gen/src/lib.rs");
    println!("cargo:rerun-if-changed=../r2r_msg_gen/src/introspection.rs");
    println!("cargo:rerun-if-changed=../r2r_msg_gen/src/native_support.rs");
    println!("cargo:rustc-check-cfg=cfg(r2r_bench_serializer)");
    println!("cargo:rustc-check-cfg=cfg(r2r_bench_reusable_typed)");
    println!("cargo:rustc-check-cfg=cfg(r2r_bench_batch_typed)");
    r2r_common::print_cargo_watches();
    // Declare all the custom cfg directives we use
    // to silence cargo warnings.
    r2r_common::print_cargo_used_cfgs(&[
        "r2r__rosgraph_msgs__msg__Clock",
        "r2r__action_msgs__msg__GoalStatus",
        "r2r__test_msgs__msg__Defaults",
        "r2r__test_msgs__msg__Arrays",
        "r2r__test_msgs__msg__WStrings",
        "r2r__example_interfaces__srv__AddTwoInts",
        "r2r__std_srvs__srv__Empty",
        "r2r__example_interfaces__action__Fibonacci",
        "r2r__type_description_interfaces__srv__GetTypeDescription",
    ]);
    r2r_common::print_cargo_ros_distro();

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    #[cfg(any(feature = "doc-only", feature = "save-bindgen"))]
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    #[cfg(any(feature = "doc-only", feature = "save-bindgen"))]
    let save_dir = r2r_common::bindings_directory(manifest_dir, env!("CARGO_PKG_NAME"));

    #[cfg(feature = "doc-only")]
    {
        println!("cargo:rerun-if-changed={}", save_dir.display());
        // If "doc-only" feature is present, copy from $crate/bindings/* to OUT_DIR
        copy_files(&save_dir, &out_dir);
        for message_cfg in fs::read_to_string(save_dir.join(CFG_FILENAME))
            .unwrap()
            .lines()
        {
            println!("cargo:rustc-check-cfg=cfg({message_cfg})");
            println!("cargo:rustc-cfg={message_cfg}");
        }
    }

    #[cfg(not(feature = "doc-only"))]
    {
        let bindgen_dir = out_dir.join("generated");
        eprintln!("Generate bindings in '{}'", bindgen_dir.display());
        generate_bindings(&bindgen_dir);

        copy_files(&bindgen_dir, &out_dir);

        #[cfg(feature = "save-bindgen")]
        {
            fs::create_dir_all(&save_dir).unwrap();
            copy_files(&bindgen_dir, &save_dir);
        }
    }
}

#[cfg(not(feature = "doc-only"))]
fn generate_bindings(bindgen_dir: &Path) {
    fs::create_dir_all(bindgen_dir).unwrap();

    let ros_distro = env::var("ROS_DISTRO").unwrap_or_default();
    let service_suffixes = r2r_common::service_message_suffixes(&ros_distro);

    let msg_list = r2r_common::get_wanted_messages();
    let msgs = r2r_common::as_map(&msg_list);

    // Persist the same interface availability used by native generation.
    // Documentation must not hide clock/action APIs just because ROS is absent.
    let mut message_cfgs: Vec<_> = msg_list
        .iter()
        .map(|msg| format!("r2r__{}__{}__{}", msg.module, msg.prefix, msg.name))
        .collect();
    message_cfgs.sort_unstable();
    message_cfgs.dedup();
    write_to_file(bindgen_dir.join(CFG_FILENAME).as_path(), message_cfgs.join("\n") + "\n")
        .unwrap();

    {
        let modules = {
            let mut modules: Vec<_> = msgs
                .par_iter()
                .map(|(module, _)| {
                    let path_suffix = format!("/{module}.rs");
                    let module_ident = format_ident!("{module}");
                    let tokens = quote! {
                        pub mod #module_ident {
                            include!(concat!(env!("OUT_DIR"), #path_suffix));
                        }
                    };
                    (module, unsafe { force_send(tokens) })
                })
                .collect();
            modules.par_sort_unstable_by_key(|(module, _)| *module);
            let modules = modules.into_iter().map(|(_, tokens)| tokens.unwrap());

            quote! { #(#modules)* }
        };

        let msgs_file = bindgen_dir.join(MSGS_FILENAME);
        write_to_file(&msgs_file, pretty_tokenstream(modules)).unwrap();
    }

    let mut mod_files: Vec<_> = msgs
        .par_iter()
        .map(|(module, prefixes)| {
            let mut snipplets: Vec<_> = prefixes
                .into_par_iter()
                .map(|(prefix, msgs)| {
                    let prefix_name = *prefix;
                    let prefix_content = match *prefix {
                        "msg" => {
                            let msg_snipplets = msgs.iter().map(|msg| {
                                println!("cargo:rustc-cfg=r2r__{}__{}__{}", module, prefix, msg);
                                r2r_msg_gen::generate_rust_msg(module, prefix, msg)
                            });

                            quote! {
                                use super::super::*;
                                #(#msg_snipplets)*
                            }
                        }
                        "srv" => {
                            let msg_snipplets = msgs.iter().map(|msg| {
                                let service_snipplet =
                                    r2r_msg_gen::generate_rust_service(module, prefix, msg);
                                let msg_snipplets = service_suffixes.iter().map(|s| {
                                    let msgname = format!("{}_{}", msg, s);
                                    println!(
                                        "cargo:rustc-cfg=r2r__{}__{}__{}",
                                        module, prefix, msg
                                    );
                                    r2r_msg_gen::generate_rust_msg(module, prefix, &msgname)
                                });
                                let msg = format_ident!("{msg}");

                                quote! {
                                    #[allow(non_snake_case)]
                                    pub mod #msg {
                                        use super::super::super::*;

                                        #service_snipplet
                                        #(#msg_snipplets)*
                                    }
                                }
                            });
                            quote! {
                                #(#msg_snipplets)*
                            }
                        }
                        "action" => {
                            let msg_snipplets = msgs.iter().map(|msg| {
                                let action_snipplet =
                                    r2r_msg_gen::generate_rust_action(module, prefix, msg);

                                let msg_snipplets =
                                    ["Goal", "Result", "Feedback"].iter().map(|s| {
                                        let msgname = format!("{}_{}", msg, s);
                                        println!(
                                            "cargo:rustc-cfg=r2r__{}__{}__{}",
                                            module, prefix, msg
                                        );
                                        r2r_msg_gen::generate_rust_msg(module, prefix, &msgname)
                                    });

                                // "internal" services that implements the action type
                                let service_snipplets =
                                    ["SendGoal", "GetResult"].iter().map(|srv| {
                                        let srvname = format!("{}_{}", msg, srv);
                                        let service_snipplet = r2r_msg_gen::generate_rust_service(
                                            module, prefix, &srvname,
                                        );

                                        let msg_snipplets = service_suffixes.iter().map(|s| {
                                            let msgname = format!("{}_{}_{}", msg, srv, s);
                                            r2r_msg_gen::generate_rust_msg(module, prefix, &msgname)
                                        });

                                        let srv = format_ident!("{srv}");
                                        quote! {
                                            #[allow(non_snake_case)]
                                            pub mod #srv {
                                                use super::super::super::super::*;

                                                #service_snipplet
                                                #(#msg_snipplets)*
                                            }
                                        }
                                    });

                                // also "internal" feedback message type that wraps the feedback type with a uuid
                                let feedback_msgname = format!("{}_FeedbackMessage", msg);
                                let feedback_msg_snipplet = r2r_msg_gen::generate_rust_msg(
                                    module,
                                    prefix,
                                    &feedback_msgname,
                                );

                                let msg = format_ident!("{msg}");
                                quote! {
                                    #[allow(non_snake_case)]
                                    pub mod #msg {
                                        use super::super::super::*;

                                        #action_snipplet
                                        #(#msg_snipplets)*
                                        #(#service_snipplets)*
                                        #feedback_msg_snipplet
                                    }
                                }
                            });

                            quote! {
                                #(#msg_snipplets)*
                            }
                        }
                        _ => {
                            panic!("unknown prefix type: {}", prefix);
                        }
                    };

                    let prefix = format_ident!("{prefix}");

                    let mod_content = quote! {
                        pub mod #prefix {
                            #prefix_content
                        }
                    };

                    (prefix_name, unsafe { force_send(mod_content) })
                })
                .collect();

            snipplets.sort_unstable_by_key(|(prefix, _)| *prefix);
            let snipplets = snipplets.into_iter().map(|(_, snipplet)| snipplet.unwrap());
            let mod_content = quote! { #(#snipplets)* };
            let file_name = format!("{}.rs", module);
            let mod_file = bindgen_dir.join(&file_name);
            write_to_file(&mod_file, pretty_tokenstream(mod_content)).unwrap();

            file_name
        })
        .collect();
    mod_files.sort_unstable();

    // Write helper files
    {
        let untyped_helper = r2r_msg_gen::generate_untyped_helper(&msg_list);
        let untyped_file = bindgen_dir.join(UNTYPED_FILENAME);
        write_to_file(&untyped_file, pretty_tokenstream(untyped_helper)).unwrap();
    }

    {
        let untyped_service_helper = r2r_msg_gen::generate_untyped_service_helper(&msg_list);
        let untyped_service_file = bindgen_dir.join(UNTYPED_SERVICE_FILENAME);
        write_to_file(&untyped_service_file, pretty_tokenstream(untyped_service_helper)).unwrap();
    }

    {
        let untyped_action_helper = r2r_msg_gen::generate_untyped_action_helper(&msg_list);
        let untyped_action_file = bindgen_dir.join(UNTYPED_ACTION_FILENAME);
        write_to_file(&untyped_action_file, pretty_tokenstream(untyped_action_helper)).unwrap();
    }

    // Save file list
    {
        let list_file = bindgen_dir.join(LIST_FILENAME);
        let mut writer = BufWriter::new(File::create(list_file).unwrap());

        for file_name in mod_files {
            writeln!(writer, "{}", file_name).unwrap();
        }

        for file_name in GENERATED_FILES {
            writeln!(writer, "{}", file_name).unwrap();
        }
    }
}

fn copy_files(src_dir: &Path, tgt_dir: &Path) {
    eprintln!("Copy files from '{}' to '{}'", src_dir.display(), tgt_dir.display());

    let src_list_file = src_dir.join(LIST_FILENAME);
    let tgt_list_file = tgt_dir.join(LIST_FILENAME);
    fs::read_to_string(&src_list_file)
        .unwrap()
        .lines()
        .for_each(|file_name| {
            let src_file = src_dir.join(file_name);
            let tgt_file = tgt_dir.join(file_name);
            fs::copy(src_file, tgt_file).unwrap();
        });

    fs::copy(&src_list_file, tgt_list_file).unwrap();
}

#[cfg(not(feature = "doc-only"))]
fn write_to_file(path: &Path, content: impl fmt::Display) -> io::Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    write!(writer, "{}", content)?;
    writer.flush()?;
    Ok(())
}

#[cfg(not(feature = "doc-only"))]
unsafe fn force_send<T>(value: T) -> force_send_sync::Send<T> {
    force_send_sync::Send::new(value)
}

#[cfg(not(feature = "doc-only"))]
fn pretty_tokenstream(stream: proc_macro2::TokenStream) -> String {
    prettyplease::unparse(&syn::parse2::<syn::File>(stream).unwrap())
}
