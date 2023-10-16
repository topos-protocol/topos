use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let descriptor_path = PathBuf::from("src/grpc/generated").join("topos.bin");

    tonic_build::configure()
        .file_descriptor_set_path(descriptor_path)
        .type_attribute(
            ".topos.shared.v1.UUID",
            "#[derive(Copy, serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.shared.v1.SubnetId",
            "#[derive(Eq, Hash, serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.shared.v1.CertificateId",
            "#[derive(Eq, Hash, serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.tce.v1.SignedReady",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.shared.v1.Positions.SourceStreamPosition",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.tce.v1.ProofOfDelivery",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.tce.v1.CheckpointResponse",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.tce.v1.CheckpointRequest",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .type_attribute(
            ".topos.tce.v1.CheckpointMapFieldEntry",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .out_dir("src/grpc/generated")
        .compile(
            &[
                "proto/topos/shared/v1/uuid.proto",
                "proto/topos/shared/v1/subnet.proto",
                "proto/topos/tce/v1/api.proto",
                "proto/topos/tce/v1/console.proto",
                "proto/topos/tce/v1/synchronization.proto",
                "proto/topos/uci/v1/certification.proto",
                "proto/topos/p2p/info.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
