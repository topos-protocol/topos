fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/generated")
        .compile(&["proto/topos/tce/v1/api.proto"], &["proto/"])?;
    Ok(())
}
