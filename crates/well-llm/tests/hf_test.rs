use well_llm::HuggingFaceClient;

#[test]
fn test_hf_image_generation() {
    let client = HuggingFaceClient::new(Some("HFAKTaLTG7AxnP69ibZamkRPAveGSmg".to_string()), "stabilityai/stable-diffusion-xl-base-1.0".to_string());
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
