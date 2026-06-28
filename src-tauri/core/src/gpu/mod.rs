//! wgpu device/context + display transform. P2 adds the compute render
//! graph (see gpu-memory-texture-lifecycle.md before extending).

pub mod display;
pub mod texture_io;

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_info: wgpu::AdapterInfo,
}

impl GpuContext {
    pub async fn init() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok_or_else(|| "no suitable GPU adapter".to_string())?;
        // Full-res RAW textures exceed the 8192 default limit (60MP ≈ 9568px)
        // — request the adapter's real limits.
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("meratech-device"),
                    required_limits: adapter.limits(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| format!("request_device: {e}"))?;
        Ok(Self {
            adapter_info: adapter.get_info(),
            device,
            queue,
        })
    }

    pub fn adapter_name(&self) -> String {
        self.adapter_info.name.clone()
    }

    pub fn backend_name(&self) -> String {
        format!("{:?}", self.adapter_info.backend)
    }
}
