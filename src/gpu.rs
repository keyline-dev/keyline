//! GPU rendering: Metal on macOS, Vulkan on Linux and Windows. Renders use
//! the GPU by default and fall back to the CPU when no GPU can be opened
//! (headless CI, containers, missing drivers).
//!
//! The CPU path stays the reference: it's byte-identical on every machine,
//! so tests and reference images use it. GPU output can differ slightly
//! between drivers.
//!
//! Opening a GPU means calling platform C APIs, so this is the one module
//! allowed `unsafe`; every block says why it holds.
#![allow(unsafe_code)]

use std::cell::RefCell;

use anyhow::{Result, anyhow, bail};
use skia_safe::{Canvas, EncodedImageFormat, ImageInfo, gpu};

/// Which rasterizer draws final renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// The GPU, falling back to the CPU when none can be opened.
    #[default]
    Gpu,
    /// Always the CPU: reproducible to the byte, used by tests.
    Cpu,
}

impl std::str::FromStr for Backend {
    type Err = anyhow::Error;

    /// `--renderer`: `gpu` or `cpu`.
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "gpu" => Ok(Backend::Gpu),
            "cpu" => Ok(Backend::Cpu),
            other => bail!("--renderer must be gpu or cpu, not {other:?}"),
        }
    }
}

/// This thread's GPU: `None` until first used, then the context, or
/// `Err` with why none could be opened (tried once per thread).
type Slot = Option<std::result::Result<Gpu, String>>;

thread_local! {
    // Skia GPU contexts aren't Send; tokio's blocking pool reuses threads,
    // so each render thread opens its GPU once.
    static GPU: RefCell<Slot> = const { RefCell::new(None) };
}

/// A GPU context and whatever must outlive it.
struct Gpu {
    context: gpu::DirectContext,
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    _vulkan: vulkan::Handles,
}

/// Draws a `w`×`h` image on this thread's GPU and returns it as PNG, or
/// `Ok(None)` when the machine has no usable GPU (callers use the CPU).
///
/// # Errors
/// When the GPU is present but drawing or reading back fails.
pub fn render_png(
    w: i32,
    h: i32,
    draw: impl FnOnce(&Canvas) -> Result<()>,
) -> Result<Option<Vec<u8>>> {
    GPU.with(|slot| {
        let mut slot = slot.borrow_mut();
        let gpu =
            slot.get_or_insert_with(|| open().ok_or_else(|| "no GPU could be opened".to_owned()));
        let Ok(gpu) = gpu else {
            return Ok(None);
        };
        let info = ImageInfo::new_n32_premul((w, h), None);
        let mut surface = gpu::surfaces::render_target(
            &mut gpu.context,
            gpu::Budgeted::Yes,
            &info,
            None,
            gpu::SurfaceOrigin::TopLeft,
            None,
            false,
            None,
        )
        .ok_or_else(|| anyhow!("can't allocate a {w}×{h} GPU surface"))?;
        draw(surface.canvas())?;
        let png = surface
            .image_snapshot()
            .encode(&mut gpu.context, EncodedImageFormat::PNG, None)
            .ok_or_else(|| anyhow!("reading the GPU render back failed"))?;
        Ok(Some(png.as_bytes().to_vec()))
    })
}

/// Whether this machine can render on the GPU (opens it on this thread).
pub fn available() -> bool {
    GPU.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| open().ok_or_else(|| "no GPU could be opened".to_owned()))
            .is_ok()
    })
}

#[cfg(target_os = "macos")]
fn open() -> Option<Gpu> {
    use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};

    let device = MTLCreateSystemDefaultDevice()?;
    let queue = device.newCommandQueue()?;
    // SAFETY: `device` and `queue` are live Metal objects for the whole
    // call; `BackendContext::new` retains both and releases them on drop,
    // and the DirectContext takes its own references.
    let backend = unsafe {
        gpu::mtl::BackendContext::new(
            objc2::rc::Retained::as_ptr(&device).cast(),
            objc2::rc::Retained::as_ptr(&queue).cast(),
        )
    };
    let context = gpu::direct_contexts::make_metal(&backend, None)?;
    Some(Gpu { context })
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn open() -> Option<Gpu> {
    let (handles, context) = vulkan::open()?;
    Some(Gpu {
        context,
        _vulkan: handles,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open() -> Option<Gpu> {
    None
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod vulkan {
    use std::ffi::c_void;

    use ash::vk::{self, Handle as _};
    use skia_safe::gpu::{self, vk::GetProcOf};

    /// The loader, instance and device the Skia context was made from; they
    /// must outlive it, so they live next to it for the thread's lifetime.
    pub struct Handles {
        _entry: ash::Entry,
        _instance: ash::Instance,
        _device: ash::Device,
    }

    /// Opens the first GPU with a graphics queue, headless (no window).
    pub fn open() -> Option<(Handles, gpu::DirectContext)> {
        // SAFETY: loads the system's Vulkan loader library; the returned
        // entry keeps it loaded for as long as it lives (inside `Handles`).
        let entry = unsafe { ash::Entry::load() }.ok()?;
        let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_1);
        let create = vk::InstanceCreateInfo::default().application_info(&app);
        // SAFETY: `create` is a valid, fully initialized create-info with no
        // layers or extensions; `app` outlives the call.
        let instance = unsafe { entry.create_instance(&create, None) }.ok()?;
        // SAFETY: `instance` is live.
        let devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
        // Prefer real GPUs; a software Vulkan (e.g. Mesa lavapipe) runs on
        // the CPU and is slower than Skia's own CPU renderer.
        let rank = |pd: vk::PhysicalDevice| {
            // SAFETY: `pd` comes from this live instance.
            match unsafe { instance.get_physical_device_properties(pd) }.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 0,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
                vk::PhysicalDeviceType::CPU => 9,
                _ => 3,
            }
        };
        let mut devices: Vec<_> = devices.into_iter().filter(|pd| rank(*pd) < 9).collect();
        devices.sort_by_key(|pd| rank(*pd));
        let (physical, family) = devices.into_iter().find_map(|pd| {
            // SAFETY: `pd` comes from this live instance.
            let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
            families
                .iter()
                .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .and_then(|i| u32::try_from(i).ok())
                .map(|i| (pd, i))
        })?;
        let priorities = [1.0];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(family)
            .queue_priorities(&priorities)];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queues);
        // SAFETY: `physical` belongs to `instance`; `device_info` and the
        // arrays it points to outlive the call.
        let device = unsafe { instance.create_device(physical, &device_info, None) }.ok()?;
        // SAFETY: queue 0 of `family` was requested in `device_info`.
        let queue = unsafe { device.get_device_queue(family, 0) };

        // Skia resolves Vulkan's entry points while making the context, so
        // the lookup (which borrows `entry` and `instance`) ends with this block.
        let context = {
            let get_proc = |of: GetProcOf| -> *const c_void {
                // SAFETY: Skia asks only for Vulkan entry points by valid,
                // NUL-terminated names, for the instance and device we created.
                let f = unsafe {
                    match of {
                        GetProcOf::Instance(i, name) => {
                            entry.get_instance_proc_addr(vk::Instance::from_raw(i as u64), name)
                        }
                        GetProcOf::Device(d, name) => {
                            instance.get_device_proc_addr(vk::Device::from_raw(d as u64), name)
                        }
                    }
                };
                f.map_or(std::ptr::null(), |f| f as *const c_void)
            };
            let builder = gpu::vk::BackendContext::new_builder(
                instance.handle().as_raw() as _,
                physical.as_raw() as _,
                device.handle().as_raw() as _,
                (queue.as_raw() as _, family as usize),
                &get_proc,
                None,
            );
            // SAFETY: every handle is live and belongs together (instance →
            // physical device → device → queue of `family`), and they outlive
            // the context because `Handles` is stored beside it.
            let backend = unsafe { builder.build() };
            gpu::direct_contexts::make_vulkan(&backend, None)
        }?;
        Some((
            Handles {
                _entry: entry,
                _instance: instance,
                _device: device,
            },
            context,
        ))
    }
}
