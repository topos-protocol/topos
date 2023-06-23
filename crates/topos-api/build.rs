use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path = PathBuf::from("src/grpc/generated").join("topos.bin");

    tonic_build::configure()
        .file_descriptor_set_path(descriptor_path)
        .type_attribute(".topos.shared.v1.UUID", "#[derive(Copy)]")
        .type_attribute(".topos.shared.v1.SubnetId", "#[derive(Eq, Hash)]")
        .type_attribute(".topos.shared.v1.CertificateId", "#[derive(Eq, Hash)]")
        .out_dir("src/grpc/generated")
        .compile(
            &[
                "proto/topos/shared/v1/uuid.proto",
                "proto/topos/shared/v1/subnet.proto",
                "proto/topos/tce/v1/api.proto",
                "proto/topos/tce/v1/console.proto",
                "proto/topos/tce/v1/synchronization.proto",
                "proto/topos/uci/v1/certification.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
