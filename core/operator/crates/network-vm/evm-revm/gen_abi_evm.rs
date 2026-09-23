use ethers_contract_abigen::Abigen;
use std::path::{Path, PathBuf};

struct Target {
    name: &'static str,
    abi_path: &'static str,
    out_file: &'static str,
}

const TARGETS: &[Target] = &[
    Target { name: "IPoWV1", abi_path: "abi/IPoWV1.abi", out_file: "ipow_v1.rs" },
    Target { name: "BetaHub", abi_path: "abi/BetaHub.abi", out_file: "beta_hub.rs" },
    Target { name: "BetaVault", abi_path: "abi/BetaVault.abi", out_file: "beta_vault.rs" },
];

fn main() {
    let out_dir = PathBuf::from("src/bindings");
    std::fs::create_dir_all(&out_dir).unwrap();

    for target in TARGETS {
        let abi_path = Path::new(target.abi_path);
        let out_file = out_dir.join(target.out_file);

        // Tell Cargo to rerun if ABI changes
        println!("cargo:rerun-if-changed={}", abi_path.display());

        // Generate if bindings file is missing OR ABI changed (Cargo triggers rerun)
        if !out_file.exists() {
            println!("Generating Rust EVM bindings for {} from ABI…", target.name);

            Abigen::new(target.name, abi_path.to_str().unwrap())
                .expect("failed to create Abigen")
                .generate()
                .expect("failed to generate bindings")
                .write_to_file(&out_file)
                .expect("failed to write bindings");

            let mut contents = std::fs::read_to_string(&out_file)
                .expect("failed to read generated bindings");

            if !contents.contains("clippy::module_inception") {
                contents = format!(
                    r#"// @generated — DO NOT EDIT
#![allow(clippy::module_inception)]

{}
"#,
                    contents
                );

                std::fs::write(&out_file, contents)
                    .expect("failed to write clippy header");
            }

            println!("Bindings written to {}", out_file.display());
        } else {
            println!("Bindings already exist for {}, skipping generation.", target.name);
        }
    }
}
