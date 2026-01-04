fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .out_dir("src/generated")
        .compile_protos(
            &[
                "llm-orchestrator/proto/orchestrator/v1/common.proto",
                "llm-orchestrator/proto/orchestrator/v1/tts.proto",
            ],
            &["llm-orchestrator/proto"],
        )?;
    Ok(())
}
