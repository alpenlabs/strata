//! Needed for MacOS to link the Python library.
//! See: https://pyo3.rs/v0.14.2/building_and_distribution.html#macos

fn main() {
    #[cfg(target_os = "macos")]
    pyo3_build_config::add_extension_module_link_args();
    #[cfg(not(target_os = "macos"))]
    ()
}
