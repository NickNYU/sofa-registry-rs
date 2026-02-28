fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/registry_model.proto",
                "proto/meta_service.proto",
                "proto/data_service.proto",
                "proto/session_service.proto",
            ],
            &["proto/"],
        )?;
    Ok(())
}
