/// This script generates rust binding from the solidity contract ABIs.
///
/// The ABIs are read from ../artifacts/contracts/...
/// and written to ./src/bindings
///
/// When debugging this script a re-run can be "forced" via
///
///     touch build.rs && cargo check -v
///
/// Appending the `-v` gives explicit information about running the build
/// script.
///
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use ethers_contract_abigen::{Abigen, ContractBindings};

struct BindingsModule {
    mod_name: String,
    bindings: ContractBindings,
}

fn remove_modules(dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if let Some(ext) = path.extension() {
            if ext == ".rs" {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // Should re-run build script if any file in the directory change, but may
    // not work as desired.
    println!("cargo:rerun-if-changed=../artifacts/contracts");

    let bindings_dir = Path::new("./src/bindings");
    remove_modules(bindings_dir)?;

    let mut binding_modules: Vec<BindingsModule> = vec![];

    for entry in fs::read_dir("../artifacts/contracts")
        .expect("Artifacts directory not found. Run `build-abi` ?")
    {
        let solidity_file_path = entry?.path();

        if !solidity_file_path.is_dir() {
            continue;
        }

        for contract in fs::read_dir(solidity_file_path.clone()).unwrap() {
            let contract = contract?;
            let abi_path = contract.path().join("abi.json");

            if abi_path.exists() {
                let contract_name = contract.file_name();

                let bindings =
                    Abigen::new(contract_name.to_str().unwrap(), abi_path.to_str().unwrap())
                        .expect("could not instantiate Abigen")
                        .generate()
                        .expect("could not generate bindings");

                let mod_name = solidity_file_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_lowercase();

                // TODO: remove skipping
                // Note: the contract ABI currently breaks compilation
                // https://github.com/gakonst/ethers-rs/issues/538
                if mod_name == "readcaptx" {
                    continue;
                }

                binding_modules.push(BindingsModule { mod_name, bindings });
            }
        }
    }

    for module in binding_modules.iter() {
        module
            .bindings
            .write_to_file(bindings_dir.join(format!("{}.rs", module.mod_name)))
            .expect("could not write bindings to file");
    }

    let mut lines = vec![];
    for module in binding_modules.iter() {
        lines.push(format!("mod {};\n", module.mod_name));
        lines.push(format!("pub use {}::*;\n", module.mod_name));
        lines.push("\n".into());
    }

    let mut writer = fs::File::create(bindings_dir.join("mod.rs"))?;
    for line in lines {
        writer
            .write_all(line.as_bytes())
            .expect("Failed to write mod.rs");
    }

    Ok(())
}
