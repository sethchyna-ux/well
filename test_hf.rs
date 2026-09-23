use well_llm::HuggingFaceClient;

fn main() {
    let client = HuggingFaceClient::new(Some("HFAKTaLTG7AxnP69ibZamkRPAveGSmg".to_string()), "stabilityai/stable-diffusion-xl-base-1.0".to_string());
    match client.generate_image("A futuristic neon terminal UI", 512, 512) {
        Ok((bytes, filename)) => {
            println!("Success! Saved to {}, {} bytes", filename, bytes.len());
            std::fs::write(&filename, bytes).unwrap();
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
