use std::{
    env, fs,
    path::{Path, PathBuf},
};

const BINDINGS_FILENAME: &str = "action_bindings.rs";

fn main() {
    r2r_common::print_cargo_watches();
    r2r_common::print_cargo_ros_distro();

    run_bindgen();
    run_dynlink();
}

fn run_bindgen() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let save_dir = manifest_dir.join("bindings");
    let target_file = out_dir.join(BINDINGS_FILENAME);
    let saved_file = save_dir.join(BINDINGS_FILENAME);

    if cfg!(feature = "doc-only") {
        // If "doc-only" feature is present, copy from $crate/bindings/* to OUT_DIR
        eprintln!("Copy from '{}' to '{}'", saved_file.display(), target_file.display());
        fs::copy(&saved_file, &target_file).unwrap();
    } else {
        eprintln!("Generate bindings file '{}'", target_file.display());
        generate_bindings(&target_file);

        #[cfg(feature = "save-bindgen")]
        {
            fs::create_dir_all(&save_dir).unwrap();
            fs::copy(&target_file, &saved_file).unwrap();
        }
    }
}

#[cfg(feature = "doc-only")]
fn run_dynlink() {}

#[cfg(not(feature = "doc-only"))]
fn run_dynlink() {
    r2r_common::print_cargo_link_search();
    println!("cargo:rustc-link-lib=dylib=rcl_action");
}

fn generate_bindings(out_file: &Path) {
    let bindings = r2r_common::setup_bindgen_builder()
        .header("src/action_wrapper.h")
        .no_debug("_OSUnaligned.*")
        .derive_partialeq(true)
        .derive_copy(true)
        // allowlist a few specific things that we need.
        .allowlist_recursively(false)
        .allowlist_type("rcl_action_client_t")
        .opaque_type("rcl_action_client_t")
        .allowlist_type("rcl_action_server_t")
        .opaque_type("rcl_action_server_t")
        .allowlist_type("rcl_action_goal_info_t")
        .allowlist_type("rcl_action_goal_handle_t")
        .opaque_type("rcl_action_goal_handle_t")
        .allowlist_type("rcl_action_cancel_request_t")
        .allowlist_type("rcl_action_cancel_request_s")
        .opaque_type("rcl_action_cancel_request_s")
        .allowlist_type("rcl_action_cancel_response_t")
        .allowlist_type("rcl_action_cancel_response_s")
        .allowlist_type("rcl_action_goal_event_t")
        .allowlist_type("rcl_action_goal_event_e")
        .allowlist_type("rcl_action_goal_state_t")
        .opaque_type("rcl_action_goal_state_t")
        .allowlist_type("rcl_action_goal_status_array_t")
        .opaque_type("rcl_action_goal_status_array_t")
        .allowlist_function("rcl_action_.*")
        .allowlist_type("rcl_action_client_options_t")
        .opaque_type("rcl_action_client_options_t")
        .allowlist_type("rcl_action_server_options_t")
        .opaque_type("rcl_action_server_options_t")
        .allowlist_var("RCL_RET_ACTION_.*")
        .generate_comments(false)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_file)
        .expect("Couldn't write bindings!");
}
