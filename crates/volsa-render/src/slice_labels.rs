//! The labels a slice is drawn with: the label volume already on the graphics device, read by
//! the slice shader as the plane shader reads it, so that the parts, or the veins found, show
//! on a slice of the Slices layout exactly as on the planes. Nothing is copied: the slice is
//! given a hold on the textures the planes and the scene draw from.

use egui_wgpu::wgpu;
use volsa_core::{Affine, Patient, Voxel};

use crate::labels::{FILL, LabelsOnDevice, Overlay};

/// The labels bound for the slice shader, and where the slice lies among them.
pub(crate) struct SliceLabels {
    layout: wgpu::BindGroupLayout,
    /// One voxel of nothing, bound where there are no labels.
    none: (wgpu::TextureView, wgpu::TextureView),
    bound: wgpu::BindGroup,
    /// The labels bound, by their number on the device; `None` for the voxel of nothing.
    number: Option<u64>,
    held: Option<LabelsOnDevice>,
    overlay: Overlay,
    /// From a pixel of the slice, as column, row and slice, to a place among the label voxels.
    to_label: Option<Affine<Voxel, Voxel>>,
}

impl SliceLabels {
    /// No labels, with the layout the slice shader reads them through as its second group.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = layout(device);
        let none = crate::plane_pipeline::no_labels(device);
        let bound = bind(device, &layout, &none.0, &none.1);
        Self {
            layout,
            none,
            bound,
            number: None,
            held: None,
            overlay: Overlay::Off,
            to_label: None,
        }
    }

    /// The layout of the group, for the pipeline.
    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The group, for drawing.
    pub(crate) fn bound(&self) -> &wgpu::BindGroup {
        &self.bound
    }

    /// Draw `with` over the slice, in the way `overlay` says, or nothing with `None`. `place`
    /// lays the slice's pixels out in the patient: the series' own placement, which takes
    /// column, row and slice. The group is made again only when other labels are given.
    pub(crate) fn set(
        &mut self,
        device: &wgpu::Device,
        with: Option<&LabelsOnDevice>,
        overlay: Overlay,
        place: Option<&Affine<Voxel, Patient>>,
    ) {
        let number = with.map(LabelsOnDevice::number);
        if number != self.number {
            let (parts, looks) = with.map_or((&self.none.0, &self.none.1), |labels| {
                (labels.parts_view(), labels.looks_view())
            });
            self.bound = bind(device, &self.layout, parts, looks);
            self.number = number;
        }
        self.held = with.cloned();
        self.overlay = overlay;
        self.to_label = with
            .zip(place)
            .map(|(labels, place)| place.then(&labels.patient_to_voxel()));
    }

    /// What the shader needs to find a pixel among the labels and to draw them: three rows
    /// from column, row and the slice's place to label voxels, the size of the label volume
    /// with how they are drawn, and which part is selected with how strongly the inside is
    /// tinted. All nothing when no labels are drawn.
    pub(crate) fn params(&self, slice: f64) -> [[f32; 4]; 5] {
        let shown = self.held.as_ref().zip(self.to_label.as_ref());
        let Some((labels, to_label)) = shown.filter(|_| self.overlay != Overlay::Off) else {
            return [[0.0; 4]; 5];
        };
        let [
            [col_u, col_v, col_w],
            [row_u, row_v, row_w],
            [on_u, on_v, on_w],
        ] = to_label.columns();
        let [start_u, start_v, start_w] = to_label.origin();
        let line = |col: f64, row: f64, on: f64, start: f64| {
            [col, row, start + on * slice, 0.0].map(single)
        };
        let [cols, rows, slices] = labels.dims().map(|side| side as f32);
        let mode = if self.overlay == Overlay::Filled {
            2.0
        } else {
            1.0
        };
        [
            line(col_u, row_u, on_u, start_u),
            line(col_v, row_v, on_v, start_v),
            line(col_w, row_w, on_w, start_w),
            [cols, rows, slices, mode],
            [labels.selected() as f32, FILL, 0.0, 0.0],
        ]
    }
}

/// The group: the part at every label voxel, then the table of looks.
fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    parts: &wgpu::TextureView,
    looks: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("volsa slice labels"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(parts),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(looks),
            },
        ],
    })
}

/// The layout of the group: a volume of whole numbers and a table of colours, read texel by
/// texel.
fn layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, sample_type, view_dimension| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled: false,
        },
        count: None,
    };
    let float = wgpu::TextureSampleType::Float { filterable: false };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("volsa slice labels"),
        entries: &[
            entry(
                0,
                wgpu::TextureSampleType::Uint,
                wgpu::TextureViewDimension::D3,
            ),
            entry(1, float, wgpu::TextureViewDimension::D2),
        ],
    })
}

/// A number as the single-precision number the shader works in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "places among label voxels are small"
)]
fn single(value: f64) -> f32 {
    value as f32
}
