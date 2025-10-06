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
        .expect(&format!("could not find package {}", EBPF_PACKAGE_NAME));

    build_ebpf([ebpf_package], Toolchain::default())
        .expect(&format!("failed to build {}", EBPF_PACKAGE_NAME))
}
