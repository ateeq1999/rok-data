//! Compiles `proto/` into the messages, the servers and the clients, with
//! protox (a protobuf compiler in Rust) in place of `protoc`.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut protos: Vec<PathBuf> = std::fs::read_dir("proto")?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    protos.retain(|path| path.extension().is_some_and(|ext| ext == "proto"));
    protos.sort();
    let descriptors = protox::compile(&protos, ["proto"])?;
    tonic_prost_build::configure().compile_fds(descriptors)?;
    println!("cargo:rerun-if-changed=proto");
    Ok(())
}
