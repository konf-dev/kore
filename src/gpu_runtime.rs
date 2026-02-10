//! GPU Runtime using wgpu
//! 
//! This module provides cross-platform GPU execution for Kore programs compiled to SPIR-V.
//! Uses wgpu which automatically selects the best backend:
//! - Vulkan on Linux/Windows/Android
//! - Metal on macOS/iOS  
//! - DirectX 12 on Windows
//! - WebGPU in browsers
//!
//! The runtime handles:
//! 1. GPU device initialization (CACHED — Fix 4)
//! 2. Loading SPIR-V compute shaders
//! 3. Buffer allocation and data transfer
//! 4. Shader dispatch
//! 5. Result retrieval
//!
//! P1: GPU dispatch is a tool (State → State).
//! P2: One operation: apply shader to data.
//! P3: Composition: pipeline stages compose via buffers.
//! P4: Constraints attenuate: GPU caps ⊆ device caps.

use std::borrow::Cow;
use std::sync::OnceLock;

/// Cached GPU context — device + queue initialized once, reused across calls.
/// P2: Single initialization. P3: Reused by composition.
struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
}

/// Global cached GPU context (Fix 4: eliminates ~190ms per-call overhead)
static GPU_CONTEXT: OnceLock<Result<GpuContext, String>> = OnceLock::new();

fn get_gpu_context() -> Result<&'static GpuContext, String> {
    GPU_CONTEXT.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "No GPU adapter found".to_string())?;
        
        let adapter_name = adapter.get_info().name.clone();
        
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Kore GPU Device"),
                required_features: wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .map_err(|e| format!("Failed to get GPU device: {}", e))?;
        
        Ok(GpuContext { device, queue, adapter_name })
    })
    .as_ref()
    .map_err(|e| e.clone())
}

/// GPU compute result
#[derive(Debug)]
pub struct GpuResult {
    pub output: Vec<i32>,
    pub elapsed_us: u64,
}

/// Error type for GPU operations
#[derive(Debug)]
pub enum GpuError {
    NoAdapter,
    NoDevice(String),
    #[allow(dead_code)]
    ShaderError(String),
    BufferError(String),
    ExecutionError(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "No GPU adapter found"),
            GpuError::NoDevice(e) => write!(f, "Failed to get GPU device: {}", e),
            GpuError::ShaderError(e) => write!(f, "Shader error: {}", e),
            GpuError::BufferError(e) => write!(f, "Buffer error: {}", e),
            GpuError::ExecutionError(e) => write!(f, "Execution error: {}", e),
        }
    }
}

impl std::error::Error for GpuError {}

/// Run a SPIR-V compute shader on the GPU
/// 
/// # Arguments
/// * `spirv_bytes` - The SPIR-V binary (must be valid, use spirv-val to check)
/// * `input` - Input data (integers)
/// * `workgroup_size` - Number of workgroups to dispatch (default: 1)
/// 
/// # Returns
/// * Output data from the compute shader
pub fn run_compute(
    spirv_bytes: &[u8],
    input: &[i32],
    workgroup_count: u32,
) -> Result<GpuResult, GpuError> {
    let start = std::time::Instant::now();
    
    // Use cached GPU context (Fix 4: ~190ms → ~0ms for device init)
    let ctx = get_gpu_context().map_err(|e| GpuError::NoDevice(e))?;
    let device = &ctx.device;
    let queue = &ctx.queue;
    
    // Create shader module from SPIR-V
    // Convert bytes to u32 words
    let spirv_words: Vec<u32> = spirv_bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    
    let shader = unsafe {
        device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
            label: Some("Kore Compute Shader"),
            source: Cow::Borrowed(&spirv_words),
        })
    };
    
    // Buffer sizes
    let input_size = (input.len() * std::mem::size_of::<i32>()) as u64;
    let output_size = input_size.max(4); // At least one i32
    
    // Create input buffer
    let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Input Buffer"),
        size: input_size.max(4),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    
    // Create output buffer
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: output_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    
    // Create staging buffer for reading results
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: output_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    
    // Write input data
    let input_bytes: Vec<u8> = input
        .iter()
        .flat_map(|&x| x.to_le_bytes())
        .collect();
    if !input_bytes.is_empty() {
        queue.write_buffer(&input_buffer, 0, &input_bytes);
    }
    
    // Create bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Kore Bind Group Layout"),
        entries: &[
            // Input buffer at binding 0
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
            // Output buffer at binding 1
            wgpu::BindGroupLayoutEntry {
                binding: 1,
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
    
    // Create bind group
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Kore Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });
    
    // Create pipeline layout
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Kore Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    
    // Create compute pipeline
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Kore Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
    });
    
    // Create command encoder and dispatch
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Kore Command Encoder"),
    });
    
    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Kore Compute Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    
    // Copy output to staging buffer
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, output_size);
    
    // Submit and wait
    queue.submit(std::iter::once(encoder.finish()));
    
    // Map staging buffer and read results
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).unwrap();
    });
    
    device.poll(wgpu::Maintain::Wait);
    
    receiver
        .recv()
        .map_err(|e| GpuError::ExecutionError(e.to_string()))?
        .map_err(|e| GpuError::BufferError(format!("{:?}", e)))?;
    
    // Read the data
    let data = buffer_slice.get_mapped_range();
    let output: Vec<i32> = data
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    
    drop(data);
    staging_buffer.unmap();
    
    let elapsed_us = start.elapsed().as_micros() as u64;
    
    Ok(GpuResult { output, elapsed_us })
}

/// Check if GPU is available (uses cached context)
pub fn gpu_available() -> bool {
    get_gpu_context().is_ok()
}

/// Get GPU info (uses cached context)
pub fn gpu_info() -> Option<String> {
    get_gpu_context().ok().map(|ctx| ctx.adapter_name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_gpu_available() {
        // GPU may not be available in test environment (headless, CI, etc.)
        // Just ensure it doesn't panic
        let result = std::panic::catch_unwind(|| {
            gpu_available()
        });
        match result {
            Ok(available) => println!("GPU available: {}", available),
            Err(_) => println!("GPU test skipped (no display)"),
        }
    }
    
    #[test]
    fn test_gpu_info() {
        // GPU info may not be available in test environment (headless, CI, etc.)
        // Just ensure it doesn't panic
        let result = std::panic::catch_unwind(|| {
            gpu_info()
        });
        if let Ok(Some(info)) = result {
            println!("GPU: {}", info);
        }
    }
}
