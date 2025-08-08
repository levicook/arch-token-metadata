use std::{env, path::PathBuf, process::Command};

fn main() {
    // Get the directory of this test crate
    let tests_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let tests_dir = PathBuf::from(&tests_dir);

    // Navigate to the anchor program directory
    let program_dir = tests_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("programs/arch-token-metadata");

    let program_manifest = program_dir.join("Cargo.toml");
    let program_src = program_dir.join("src");

    println!("cargo:rerun-if-changed={}", program_manifest.display());
    println!("cargo:rerun-if-changed={}", program_src.display());

    let sbf_out_dir = std::env::temp_dir().join("arch-token-metadata-bin");
    let program_so_path = sbf_out_dir.join("arch_token_metadata.so");

    // Build the program using cargo build-sbf
    let output = Command::new("cargo")
        .args([
            "build-sbf",
            "--manifest-path",
            &program_manifest.to_string_lossy(),
            "--sbf-out-dir",
            &sbf_out_dir.to_string_lossy(),
        ])
        .output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                eprintln!("Failed to build Anchor program:");
                eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
                eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                std::process::exit(1);
            }
            println!("Successfully built Anchor program");
        }
        Err(e) => {
            eprintln!("Failed to execute cargo build-sbf: {e}");
            eprintln!("Make sure you have the Solana CLI tools installed");
            std::process::exit(1);
        }
    }

    // Set environment variable for this program
    println!(
        "cargo:rustc-env=ARCH_TOKEN_METADATA_SO={}",
        program_so_path.display()
    );
}
