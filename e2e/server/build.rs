fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(false)
        // reuse the canonical generated types instead of regenerating them
        .extern_path(
            ".ibc.lightclients.ethereum.v1",
            "::ethereum_light_client_proto::ibc::lightclients::ethereum::v1",
        )
        .extern_path(
            ".ibc.core.client.v1",
            "::ethereum_light_client_proto::ibc::core::client::v1",
        )
        .compile(
            &["../proto/e2e/v1/e2e.proto"],
            &[
                "../proto",
                "../../proto/definitions",
                "../proto/third_party",
            ],
        )?;
    Ok(())
}
