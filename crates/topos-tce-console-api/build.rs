fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/generated")
        .compile(&["proto/instrument.proto"], &["proto"])?;
    Ok(())
}
