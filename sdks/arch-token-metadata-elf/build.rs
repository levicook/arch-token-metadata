use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));

    // If the workspace program is present, rebuild automatically (one-step dev workflow)
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent());
    if let Some(root) = workspace_root {
        let program_dir = root.join("programs/arch-token-metadata");
        let program_manifest = program_dir.join("Cargo.toml");
        let program_src = program_dir.join("src");
        if program_manifest.exists() {
            println!("cargo:rerun-if-changed={}", program_manifest.display());
            println!("cargo:rerun-if-changed={}", program_src.display());
            // Always write the built artifact into the packaged path used by this crate
            let packaged_dir = manifest_dir.join("elf");
            let program_so_path = packaged_dir.join("arch_token_metadata.so");
            if let Err(e) = fs::create_dir_all(&packaged_dir) {
                eprintln!("Failed to create packaged elf dir: {e}");
                std::process::exit(1);
            }

            let output = Command::new("cargo")
                .args([
                    "build-sbf",
                    "--manifest-path",
                    &program_manifest.to_string_lossy(),
                    "--sbf-out-dir",
                    &packaged_dir.to_string_lossy(),
                ])
                .output();

            match output {
                Ok(output) => {
                    if !output.status.success() {
                        eprintln!("Failed to build program:");
                        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to execute cargo build-sbf: {e}");
                    eprintln!("Ensure Solana CLI tools are installed.");
                    std::process::exit(1);
                }
            }

            println!(
                "cargo:rustc-env=ARCH_TOKEN_METADATA_SO={}",
                program_so_path.display()
            );
            return;
        }
    }

    // Otherwise, use packaged ELF shipped in this crate (publish-friendly)
    let packaged = manifest_dir.join("elf/arch_token_metadata.so");
    println!("cargo:rerun-if-changed={}", packaged.display());
    if packaged.exists() {
        println!(
            "cargo:rustc-env=ARCH_TOKEN_METADATA_SO={}",
            packaged.display()
        );
        return;
    }

    eprintln!("Missing packaged ELF at: {}", packaged.display());
    eprintln!(
        "To fix: copy your built .so to 'elf/arch_token_metadata.so' or run in the workspace with the program present."
    );
    std::process::exit(1);
}
