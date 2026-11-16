//! Short, localized descriptions of the work reported by agent tool calls.
use benchup_core::tr;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Activity {
    Reading,
    Finding,
    Creating,
    Updating,
    Materials,
    Joints,
    Deleting,
    Selecting,
    Checking,
    Estimating,
    Rendering,
    Batch,
    Preview,
    Undo,
    Redo,
    Working,
}

impl Activity {
    pub(super) fn for_call(name: &str, arguments: &Value) -> Self {
        match name {
            "run_batch" if arguments.get("dry_run").and_then(Value::as_bool) == Some(true) => Self::Preview,
            "run_batch" => {
                let calls = arguments.get("calls").and_then(Value::as_array);
                if let Some(calls) = calls.filter(|calls| calls.len() == 1) {
                    // Single-call batches are how the CLI forwards ordinary calls.
                    // Do not recurse into untrusted nested batches.
                    let call = &calls[0];
                    Self::for_tool(call.get("tool").and_then(Value::as_str).unwrap_or_default())
                } else {
                    Self::Batch
                }
            }
            "undo" if arguments.get("redo").and_then(Value::as_bool) == Some(true) => Self::Redo,
            "undo" => Self::Undo,
            name => Self::for_tool(name),
        }
    }

    fn for_tool(name: &str) -> Self {
        match name {
            "get_scene" | "get_object" | "get_selection" | "list_stock_materials" | "get_guide" | "measure_space" => Self::Reading,
            "find_objects" | "find_surfaces" => Self::Finding,
            "create_board" | "create_profile_board" | "create_copy" | "draw_polyline" | "add_dimension" | "add_text" => Self::Creating,
            "move" | "move_to" | "rotate" | "scale" | "resize_board" | "push_pull" | "rename" | "group" | "ungroup"
            | "tag" | "split_face" | "merge_faces" | "change_thickness" | "distribute" => Self::Updating,
            "create_stock_material" | "set_material" | "set_grain" | "paint" | "set_finish" => Self::Materials,
            "add_joint" | "remove_joint" | "flip_joint" | "set_joint" | "add_profile" | "remove_profile" => Self::Joints,
            "erase" => Self::Deleting,
            "select" | "enter_object" | "exit_to_root" => Self::Selecting,
            "check_model" => Self::Checking,
            "get_cut_list" | "get_estimate" => Self::Estimating,
            "render_view" => Self::Rendering,
            _ => Self::Working,
        }
    }

    pub(super) fn viewport_mode(self) -> crate::renderer::agent_effects::Mode {
        use crate::renderer::agent_effects::Mode;
        match self {
            Self::Reading | Self::Finding | Self::Selecting | Self::Checking
            | Self::Estimating | Self::Rendering => Mode::Looking,
            _ => Mode::Preparing,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Reading => tr!("agent_link.activity.reading"),
            Self::Finding => tr!("agent_link.activity.finding"),
            Self::Creating => tr!("agent_link.activity.creating"),
            Self::Updating => tr!("agent_link.activity.updating"),
            Self::Materials => tr!("agent_link.activity.materials"),
            Self::Joints => tr!("agent_link.activity.joints"),
            Self::Deleting => tr!("agent_link.activity.deleting"),
            Self::Selecting => tr!("agent_link.activity.selecting"),
            Self::Checking => tr!("agent_link.activity.checking"),
            Self::Estimating => tr!("agent_link.activity.estimating"),
            Self::Rendering => tr!("agent_link.activity.rendering"),
            Self::Batch => tr!("agent_link.activity.batch"),
            Self::Preview => tr!("agent_link.activity.preview"),
            Self::Undo => tr!("agent_link.activity.undo"),
            Self::Redo => tr!("agent_link.activity.redo"),
            Self::Working => tr!("agent_link.activity.working"),
        }
    }
}
