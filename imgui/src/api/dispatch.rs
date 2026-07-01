use godot::prelude::*;
use imgui::sys;

use super::ImGuiApi;
use crate::backend::is_in_frame;

fn emit_with(signal: &Signal, value: Variant, extra: &VarArray) {
    let mut args = Vec::with_capacity(1 + extra.len());
    args.push(value);
    args.extend(extra.iter_shared());
    signal.emit(&args);
}

#[godot_api(secondary)]
impl ImGuiApi {
    /// Emit `signal` with `value` when the widget just drawn was changed.
    ///
    /// Call this immediately after the widget with the changing value
    ///
    /// The signal fires once per edit, on the frame the edit is committed (for a
    /// slider or drag: when the mouse is released, for a text field: when it loses
    /// focus, etc.), so the signal only emits when the value settles rather than on every
    /// frame.
    ///
    /// ```gdscript
    /// brightness = ImGui.slider_float("brightness", brightness, 0.0, 1.0)
    /// ImGui.dispatch_on_changed(brightness, brightness_changed)
    /// ```
    #[func]
    fn dispatch_on_changed(&self, value: Variant, signal: Signal) {
        if is_in_frame() && unsafe { sys::igIsItemDeactivatedAfterEdit() } {
            signal.emit(&[value]);
        }
    }

    /// Like `dispatch_on_changed`, but appends `extra_args` to the emitted arguments.
    ///
    /// The signal is emitted as `value` followed by each element of `extra_args`.
    ///
    /// ```gdscript
    /// cfg.volume = ImGui.slider_float("volume", cfg.volume, 0.0, 1.0)
    /// ImGui.dispatch_on_changed_ex(cfg.volume, setting_changed, ["volume", true, 650])
    /// 
    /// # handler:
    /// setting_changed.connect(func(value: Variant, setting: String, extra_b: bool, extra_c: int):
    ///     # ...
    /// )
    /// ```
    #[func]
    fn dispatch_on_changed_ex(&self, value: Variant, signal: Signal, extra_args: VarArray) {
        if is_in_frame() && unsafe { sys::igIsItemDeactivatedAfterEdit() } {
            emit_with(&signal, value, &extra_args);
        }
    }

    /// Emit `signal` with `value` on every frame the widget just drawn is being changed.
    ///
    /// Call this immediately after the widget with the changing value, before any
    /// other widget call.
    ///
    /// Unlike `dispatch_on_changed`, the signal fires at each instant the value is updated 
    /// (for a slider or drag: on each frame it is dragged).
    ///
    /// ```gdscript
    /// brightness = ImGui.slider_float("brightness", brightness, 0.0, 1.0)
    /// ImGui.dispatch_on_changing(brightness, brightness_changing)
    /// ```
    #[func]
    fn dispatch_on_changing(&self, value: Variant, signal: Signal) {
        if is_in_frame() && unsafe { sys::igIsItemEdited() } {
            signal.emit(&[value]);
        }
    }

    /// Like `dispatch_on_changing`, but appends `extra_args` to the emitted arguments.
    ///
    /// The signal is emitted as `value` followed by each element of `extra_args`
    ///
    /// ```gdscript
    /// cfg.volume = ImGui.slider_float("volume", cfg.volume, 0.0, 1.0)
    /// ImGui.dispatch_on_changing_ex(cfg.volume, setting_changing, ["volume", true, 650])
    /// 
    /// # handler:
    /// setting_changing.connect(func(value: Variant, setting: String, extra_b: bool, extra_c: int):
    ///     # ...
    /// )
    #[func]
    fn dispatch_on_changing_ex(&self, value: Variant, signal: Signal, extra_args: VarArray) {
        if is_in_frame() && unsafe { sys::igIsItemEdited() } {
            emit_with(&signal, value, &extra_args);
        }
    }
}
