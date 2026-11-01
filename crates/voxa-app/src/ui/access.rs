//! Telling a screen reader what a control is called, for the controls whose own words do not
//! say it alone: a field with only a hint in it, a button of one row among many that all say
//! the same thing.

use egui::Response;

/// Call the control behind `response` `name` for a screen reader. The words the control draws
/// stay as they are.
pub fn call(response: &Response, name: impl Into<Box<str>>) {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(name));
}
