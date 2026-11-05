use well_llm::HuggingFaceClient;

#[test]
fn test_hf_image_generation() {
    let Some(token) = std::env::var("HF_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
    else {
        eprintln!("Skipping live Hugging Face image generation test; set HF_TOKEN to run it.");
        return;
    };

    let client = HuggingFaceClient::new(
        Some(token),
        "stabilityai/stable-diffusion-xl-base-1.0".to_string(),
    );
    match client.generate_image("A futuristic neon terminal UI", 512, 512) {
        Ok((bytes, filename)) => {
            println!("Success! Saved to {}, {} bytes", filename, bytes.len());
            assert!(!bytes.is_empty());
        }
        Err(e) => {
            println!("Error: {}", e);
            panic!("Failed to generate image: {}", e);
        }
    }
}
