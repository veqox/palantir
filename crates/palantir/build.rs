use aya_build::{
    Toolchain, build_ebpf,
    cargo_metadata::{self, Metadata, Package},
};

const EBPF_PACKAGE_NAME: &str = "palantir-ebpf";

fn main() {
    let Metadata { packages, .. } = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("failed to run metadata command");

    let ebpf_package = packages
        .into_iter()
        .find(|Package { name, .. }| name.as_str() == EBPF_PACKAGE_NAME)
        .unwrap_or_else(|| panic!("could not find package {}", EBPF_PACKAGE_NAME));

    build_ebpf([ebpf_package], Toolchain::default())
        .unwrap_or_else(|_| panic!("failed to build {}", EBPF_PACKAGE_NAME))
}
