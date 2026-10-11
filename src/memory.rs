//! Big blocks go back to the system when freed. A video's frames are a few
//! full-size buffers each, freed as soon as they're encoded; glibc, left
//! to itself, starts keeping blocks that size inside each thread's arena,
//! where nothing hands them back, and an idle server held ~100 MB after
//! one video on one core, of which ~20 MB was in use.
//! Besides `gpu`, the one module allowed `unsafe`: a single C call.
#![allow(unsafe_code)]

/// Blocks this big or bigger are mapped on their own and unmapped when
/// freed: every frame buffer, none of the small allocations.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const OWN_MAPPING: libc::c_int = 1 << 20;

/// Makes freed frame buffers go back to the system; call once at start.
pub fn give_back_big_blocks() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: `mallopt` takes two plain integers and only sets a value in
    // the allocator's own parameters, under its lock; any thread, any time.
    unsafe {
        libc::mallopt(libc::M_MMAP_THRESHOLD, OWN_MAPPING);
    }
}
