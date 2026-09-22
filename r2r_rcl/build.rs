use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=src/rcl_wrapper.h");
    r2r_common::print_cargo_watches();
    r2r_common::print_cargo_ros_distro();
    run_bindgen();
    run_dynlink();
}

fn run_bindgen() {
    let out_dir: PathBuf = env::var_os("OUT_DIR").unwrap().into();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target_file = out_dir.join("rcl_bindings.rs");
    let saved_file = manifest_dir.join("bindings").join("rcl_bindings.rs");
    if cfg!(feature = "doc-only") {
        println!("cargo:rerun-if-changed={}", saved_file.display());
    }

    // Use saved bindings file if "docs-only" feature is enabled.
    let src_file = if cfg!(feature = "doc-only") {
        &saved_file
    } else {
        eprintln!("Generate bindings file '{}'", target_file.display());
        gen_bindings(&target_file);

        #[cfg(feature = "save-bindgen")]
        {
            if let Some(dir) = saved_file.parent() {
                fs::create_dir_all(dir)
                    .unwrap_or_else(|_| panic!("Unable to create directory '{}'", dir.display()));
            }
            fs::copy(&target_file, &saved_file).expect("File copy failed");
        }

        &target_file
    };

    if src_file != &target_file {
        fs::copy(src_file, &target_file).unwrap_or_else(|_| {
            panic!("Unable to copy from '{}' to '{}'", src_file.display(), target_file.display())
        });
    }
}

#[cfg(feature = "doc-only")]
fn run_dynlink() {}

#[cfg(not(feature = "doc-only"))]
fn run_dynlink() {
    r2r_common::print_cargo_link_search();
    println!("cargo:rustc-link-lib=dylib=rcl");
    println!("cargo:rustc-link-lib=dylib=rcl_logging_spdlog");
    println!("cargo:rustc-link-lib=dylib=rcl_yaml_param_parser");
    println!("cargo:rustc-link-lib=dylib=rcutils");
    println!("cargo:rustc-link-lib=dylib=rmw");
    println!("cargo:rustc-link-lib=dylib=rmw_implementation");
    println!("cargo:rustc-link-lib=dylib=rosidl_typesupport_c");
    println!("cargo:rustc-link-lib=dylib=rosidl_runtime_c");
}

fn gen_bindings(out_file: &Path) {
    if let Some(dir) = out_file.parent() {
        fs::create_dir_all(dir)
            .unwrap_or_else(|_| panic!("Unable to create directory '{}'", dir.display()));
    }

    let bindings = r2r_common::setup_bindgen_builder()
        .header("src/rcl_wrapper.h")
        .allowlist_type("rcl_.*")
        .allowlist_type("rcutils_.*")
        .allowlist_type("rmw_.*")
        .allowlist_type("rosidl_.*")
        .allowlist_type("RCUTILS_.*")
        .allowlist_var("RCL_.*")
        .allowlist_var("RCUTILS_.*")
        .allowlist_var("RMW_.*")
        .allowlist_var("rosidl_.*")
        .allowlist_var("g_rcutils_.*")
        .allowlist_function("rcl_.*")
        .allowlist_function("rcutils_.*")
        .allowlist_function("rmw_.*")
        .allowlist_function("rosidl_.*")
        .allowlist_function(".*_typesupport_.*")
        .allowlist_function(".*_sequence_bound_.*")
        .no_debug("_OSUnaligned.*")
        .derive_copy(true)
        .generate_comments(false)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_file)
        .expect("Couldn't write bindings!");
}
