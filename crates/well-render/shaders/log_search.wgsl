@group(0) @binding(0) var<storage, read> text_buffer: array<u32>; // Packed 4 bytes per u32
@group(0) @binding(1) var<storage, read> automaton_table: array<u32>; // [state * 256 + char_byte]
@group(0) @binding(2) var<uniform> buffer_len: u32; // length in bytes
@group(0) @binding(3) var<storage, read_write> histogram: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> bitmask_buffer: array<atomic<u32>>; // bitmask array for hits

const BLOCK_SIZE: u32 = 4096u; // bytes per histogram bucket
const CHUNK_SIZE: u32 = 64u; // bytes per bit in bitmask

fn get_byte(index: u32) -> u32 {
    let word_idx = index / 4u;
    let byte_idx = index % 4u;
    let word = text_buffer[word_idx];
    return (word >> (byte_idx * 8u)) & 0xFFu;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let start_idx = global_id.x;
    
    if (start_idx >= buffer_len) {
        return;
    }
    
    // Each thread starts Aho-Corasick traversal from its assigned byte position
    // We traverse up to a bounded depth or until a match is found.
    // In standard Aho-Corasick, usually you traverse sequentially over the whole text on CPU.
    // On GPU, mapping a thread per byte to see if a word ENDS at this byte or STARTS at this byte is better.
    // We will see if any pattern STARTS at start_idx.
    
    var current_state = 0u;
    var match_found = false;
    
    // We check a window of bytes from start_idx. 
    // To prevent infinite loops, we cap the search window to an arbitrary max pattern length (e.g. 256).
    for (var i = 0u; i < 256u; i = i + 1u) {
        let text_idx = start_idx + i;
        if (text_idx >= buffer_len) {
            break;
        }
        
        let c = get_byte(text_idx);
        let table_idx = current_state * 256u + c;
        let next_val = automaton_table[table_idx];
        
        let next_state = next_val & 0x7FFFFFFFu;
        let is_match = (next_val & 0x80000000u) != 0u;
        
        if (is_match) {
            match_found = true;
            break;
        }
        
        if (next_state == 0u) {
            // failed transition and we are back to root (or dead end)
            // since we are checking if a match *starts* at start_idx, a failure means no match starts here.
            break;
        }
        
        current_state = next_state;
    }
    
    if (match_found) {
        // Accumulate to global density histogram
        let bucket = start_idx / BLOCK_SIZE;
        atomicAdd(&histogram[bucket], 1u);
        
        // Write to compact bitmask
        let bitmask_idx = start_idx / CHUNK_SIZE;
        let word_idx = bitmask_idx / 32u;
        let bit_idx = bitmask_idx % 32u;
        atomicOr(&bitmask_buffer[word_idx], 1u << bit_idx);
    }
}
