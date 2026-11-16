//! The shaders the slices and the planes draw with, compiled once for the window's device and
//! shared by every viewport and every plane panel, however many there are.

use egui_wgpu::RenderState;
use volsa_render::{PlaneShader, SliceShader};

/// The slice shader for `render`'s device, compiled the first time it is asked for.
pub(super) fn slice(render: &RenderState) -> SliceShader {
    shared(render, || {
        SliceShader::new(&render.device, render.target_format)
    })
}

/// The plane shader for `render`'s device, compiled the first time it is asked for.
pub(super) fn plane(render: &RenderState) -> PlaneShader {
    shared(render, || {
        PlaneShader::new(&render.device, render.target_format)
    })
}

/// The one `T` kept with the device's renderer, made by `make` if there is none yet.
fn shared<T: Clone + Send + Sync + 'static>(render: &RenderState, make: impl FnOnce() -> T) -> T {
    let mut renderer = render.renderer.write();
    let kept = &mut renderer.callback_resources;
    if let Some(found) = kept.get::<T>() {
        return found.clone();
    }
    let new = make();
    kept.insert(new.clone());
    new
}
