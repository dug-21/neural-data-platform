use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("proto");
    
    let proto_files = std::fs::read_dir(&proto_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "proto" {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    
    if proto_files.is_empty() {
        println!("cargo:warning=No .proto files found in {:?}", proto_dir);
        return Ok(());
    }
    
    for proto_file in &proto_files {
        println!("cargo:rerun-if-changed={}", proto_file.display());
    }
    
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/proto")
        .compile(&proto_files, &[proto_dir])?;
    
    println!("cargo:rerun-if-changed=proto/");
    Ok(())
}
