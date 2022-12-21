use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path = PathBuf::from("src/generated").join("topos.bin");

    tonic_build::configure()
        .file_descriptor_set_path(descriptor_path)
        .out_dir("src/generated")
        .compile(
            &[
                "proto/topos/shared/v1/uuid.proto",
                "proto/topos/shared/v1/subnet.proto",
                "proto/topos/tce/v1/api.proto",
                "proto/topos/tce/v1/console.proto",
                "proto/topos/uci/v1/certification.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
