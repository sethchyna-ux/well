use std::sync::Arc;
use wgpu::util::DeviceExt;

const CHUNK_SIZE_BYTES: usize = 64 * 1024 * 1024; // 64MB rolling window
const BLOCK_SIZE_BYTES: usize = 4096;
const MAX_HISTOGRAM_BUCKETS: usize = CHUNK_SIZE_BYTES / BLOCK_SIZE_BYTES;
const BITMASK_CHUNK_SIZE: usize = 64; // bits per 64 bytes
const BITMASK_U32_COUNT: usize = (CHUNK_SIZE_BYTES / BITMASK_CHUNK_SIZE) / 32;

pub struct HephaestusEngine {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    histogram_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    bitmask_buffer: wgpu::Buffer,
    bitmask_staging: wgpu::Buffer,
}

impl HephaestusEngine {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Log Search Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/log_search.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Hephaestus Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Hephaestus Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Hephaestus Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let histogram_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Histogram Buffer"),
            size: (MAX_HISTOGRAM_BUCKETS * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Histogram Staging Buffer"),
            size: (MAX_HISTOGRAM_BUCKETS * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bitmask_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bitmask Buffer"),
            size: (BITMASK_U32_COUNT * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bitmask_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bitmask Staging Buffer"),
            size: (BITMASK_U32_COUNT * 4) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            histogram_buffer,
            staging_buffer,
            bitmask_buffer,
            bitmask_staging,
        }
    }

    /// Compiles search patterns into a 1D Aho-Corasick state transition table.
    /// Format: `[state * 256 + char] = next_state | (is_match << 31)`.
    pub fn build_automaton(patterns: &[&str]) -> Vec<u32> {
        let mut trie: Vec<[u32; 256]> = vec![[0; 256]];
        let mut match_states = vec![false];

        for pattern in patterns {
            let mut current_state = 0;
            for b in pattern.bytes() {
                let b_lower = b.to_ascii_lowercase();

                if trie[current_state][b_lower as usize] == 0 {
                    trie[current_state][b_lower as usize] = trie.len() as u32;
                    let new_state = [0; 256];
                    trie.push(new_state);
                    match_states.push(false);
                }

                // Case-insensitive mapping for Aho-Corasick on ASCII
                let next_state = trie[current_state][b_lower as usize];
                if b.is_ascii_uppercase() {
                    trie[current_state][b as usize] = next_state;
                } else if b.is_ascii_lowercase() {
                    trie[current_state][(b - 32) as usize] = next_state;
                }

                current_state = next_state as usize;
            }
            match_states[current_state] = true;
        }

        // We use a simple trie without failure links for exact string matching from any starting position.
        // The compute shader checks each starting position individually.
        let mut table = vec![0u32; trie.len() * 256];
        for state in 0..trie.len() {
            for c in 0..256 {
                let mut val = trie[state][c];
                if match_states[val as usize] {
                    val |= 0x80000000;
                }
                table[state * 256 + c] = val;
            }
        }

        table
    }

    pub fn upload_stream_chunk(&self, chunk_data: &[u8], patterns: &[&str], clear_histogram: bool) {
        let padded_len = (chunk_data.len() + 3) & !3;
        let mut padded_data = vec![0u8; padded_len];
        padded_data[..chunk_data.len()].copy_from_slice(chunk_data);

        let text_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Text Buffer"),
                contents: &padded_data,
                usage: wgpu::BufferUsages::STORAGE,
            });

        let buffer_len_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Buffer Length"),
                contents: bytemuck::cast_slice(&[chunk_data.len() as u32]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let automaton = Self::build_automaton(patterns);
        let automaton_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Automaton Table"),
                contents: bytemuck::cast_slice(&automaton),
                usage: wgpu::BufferUsages::STORAGE,
            });

        if clear_histogram {
            let zero_histogram = vec![0u32; MAX_HISTOGRAM_BUCKETS];
            self.queue.write_buffer(
                &self.histogram_buffer,
                0,
                bytemuck::cast_slice(&zero_histogram),
            );
        }

        // Always clear bitmask for the current chunk
        let zero_bitmask = vec![0u32; BITMASK_U32_COUNT];
        self.queue
            .write_buffer(&self.bitmask_buffer, 0, bytemuck::cast_slice(&zero_bitmask));

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hephaestus Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: text_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: automaton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_len_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.histogram_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.bitmask_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let workgroups = (chunk_data.len() as u32).div_ceil(256);
            cpass.dispatch_workgroups(workgroups, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            &self.histogram_buffer,
            0,
            &self.staging_buffer,
            0,
            self.histogram_buffer.size(),
        );

        encoder.copy_buffer_to_buffer(
            &self.bitmask_buffer,
            0,
            &self.bitmask_staging,
            0,
            self.bitmask_buffer.size(),
        );

        self.queue.submit(Some(encoder.finish()));
    }

    pub async fn poll_heatmap(&self) -> (Vec<u32>, Vec<u32>) {
        let (sender_hist, receiver_hist) = std::sync::mpsc::channel();
        let (sender_bit, receiver_bit) = std::sync::mpsc::channel();

        let hist_slice = self.staging_buffer.slice(..);
        hist_slice.map_async(wgpu::MapMode::Read, move |v| sender_hist.send(v).unwrap());

        let bit_slice = self.bitmask_staging.slice(..);
        bit_slice.map_async(wgpu::MapMode::Read, move |v| sender_bit.send(v).unwrap());

        self.device.poll(wgpu::Maintain::Wait);

        let mut hist_result = vec![];
        let mut bit_result = vec![];

        if let Ok(Ok(())) = receiver_hist.recv() {
            let data = hist_slice.get_mapped_range();
            hist_result = bytemuck::cast_slice(&data).to_vec();
            drop(data);
            self.staging_buffer.unmap();
        }

        if let Ok(Ok(())) = receiver_bit.recv() {
            let data = bit_slice.get_mapped_range();
            bit_result = bytemuck::cast_slice(&data).to_vec();
            drop(data);
            self.bitmask_staging.unmap();
        }

        (hist_result, bit_result)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn log_search_shader_passes_wgpu_validation() {
        let source = include_str!("../shaders/log_search.wgsl");
        let module = naga::front::wgsl::parse_str(source).expect("WGSL should parse");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );

        validator
            .validate(&module)
            .expect("WGSL should pass naga validation");
    }
}
