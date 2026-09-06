//! Export tile path — linear f16 readback without display transform.

use super::FinalTag;
use super::RenderGraph;
use crate::doc::EditDoc;
use crate::error::CoreError;
use crate::gpu::display::ViewParams;
use crate::gpu::texture_io::readback_rgba16f_from_texture;
use crate::gpu::GpuContext;
use crate::profile::DcpProfile;
use std::collections::HashMap;

impl RenderGraph {
    /// Export path (contract F2 output side): run the full chain (modules
    /// plus masks, no display transform) for one tile and read back LINEAR
    /// Rec.2020 f32. Tiled by the caller (gpu-memory spec §5: export is
    /// sequential and bounded). Caching is bypassed — every call re-runs.
    ///
    /// DCP must run here (same as the viewport: extract → DCP → modules).
    /// Applying `apply_look` after the module chain stacked the tone curve
    /// on already-edited pixels and made Camera exports diverge from the
    /// editor.
    #[allow(clippy::too_many_arguments)]
    pub fn render_linear_tile(
        &mut self,
        gpu: &GpuContext,
        working_view: &wgpu::TextureView,
        img_w: u32,
        img_h: u32,
        view: &ViewParams,
        doc: &EditDoc,
        as_shot_cct: f32,
        seg_masks: &HashMap<String, wgpu::TextureView>,
        lut: Option<&crate::lut::CubeLut>,
        dcp_profile: Option<&DcpProfile>,
    ) -> Result<Vec<f32>, CoreError> {
        self.invalidate_all();
        // run the normal render to execute the whole chain (present output
        // is discarded; cheap relative to the chain itself)…
        // The look LUT is an in-chain creative module, so it bakes into the
        // linear readback here just like grade/hsl/curve. DCP is in-chain too
        // (extract → look_tex → modules) — same order as the viewport.
        let _ = self.render(
            gpu,
            working_view,
            img_w,
            img_h,
            view,
            doc,
            as_shot_cct,
            seg_masks,
            None,
            dcp_profile,
            lut,
        )?;
        // …then read the LINEAR texture that fed present: the last
        // non-identity stage output (or extract / DCP look when defaults).
        let out_w = view.out_w.max(1);
        let out_h = view.out_h.max(1);
        let final_tex = match self.last_final {
            FinalTag::Extract => self.extract_tex.as_ref().unwrap(),
            FinalTag::Look => self.look_tex.as_ref().expect("dcp look tex"),
            FinalTag::Node(i) => self.node_tex[i].as_ref().unwrap(),
            FinalTag::Comp(i) => &self.composite[i],
        };
        readback_rgba16f_from_texture(gpu, final_tex, out_w, out_h)
    }
}
