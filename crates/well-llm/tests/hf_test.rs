use well_llm::HuggingFaceClient;

#[test]
#[ignore = "requires HF_TOKEN and network access to Hugging Face"]
fn test_hf_image_generation() {
    let token = std::env::var("HF_TOKEN").expect("set HF_TOKEN to run this integration test");
    let client = HuggingFaceClient::new(
        Some(token),
        "stabilityai/stable-diffusion-xl-base-1.0".to_string(),
    );
    match client.generate_image("A futuristic neon terminal UI", 512, 512) {
        Ok((bytes, filename)) => {
            println!("Success! Saved to {}, {} bytes", filename, bytes.len());
            // std::fs::write(&filename, bytes).unwrap();
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Failed to generate image: {}", e);
        }
    }
}
