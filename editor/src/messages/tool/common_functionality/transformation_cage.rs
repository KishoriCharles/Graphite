use crate::consts::{BOUNDS_ROTATE_THRESHOLD, BOUNDS_SELECT_THRESHOLD, SELECTION_DRAG_ANGLE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::transformation::OriginalTransforms;
use crate::messages::prelude::*;

use graphene_core::renderer::Quad;

use glam::{DAffine2, DVec2};

/// Contains the edges that are being dragged along with the original bounds.
#[derive(Clone, Debug, Default)]
pub struct SelectedEdges {
	bounds: [DVec2; 2],
	top: bool,
	bottom: bool,
	left: bool,
	right: bool,
	// Aspect ratio in the form of width/height, so x:1 = width:height
	aspect_ratio: f64,
}

impl SelectedEdges {
	pub fn new(top: bool, bottom: bool, left: bool, right: bool, bounds: [DVec2; 2]) -> Self {
		let size = (bounds[0] - bounds[1]).abs();
		let aspect_ratio = size.x / size.y;
		Self {
			top,
			bottom,
			left,
			right,
			bounds,
			aspect_ratio,
		}
	}

	/// Calculate the pivot for the operation (the opposite point to the edge dragged)
	pub fn calculate_pivot(&self) -> DVec2 {
		self.pivot_from_bounds(self.bounds[0], self.bounds[1])
	}

	fn pivot_from_bounds(&self, min: DVec2, max: DVec2) -> DVec2 {
		let x = if self.left {
			max.x
		} else if self.right {
			min.x
		} else {
			(min.x + max.x) / 2.
		};

		let y = if self.top {
			max.y
		} else if self.bottom {
			min.y
		} else {
			(min.y + max.y) / 2.
		};

		DVec2::new(x, y)
	}

	/// Computes the new bounds with the given mouse move and modifier keys
	pub fn new_size(&self, mouse: DVec2, transform: DAffine2, center: bool, center_around: DVec2, constrain: bool) -> (DVec2, DVec2) {
		let mouse = transform.inverse().transform_point2(mouse);

		let mut min = self.bounds[0];
		let mut max = self.bounds[1];
		if self.top {
			min.y = mouse.y;
		} else if self.bottom {
			max.y = mouse.y;
		}
		if self.left {
			min.x = mouse.x;
		} else if self.right {
			max.x = mouse.x;
		}

		let mut pivot = self.pivot_from_bounds(min, max);
		if center {
			// The below ratio is: `dragging edge / being centered`.
			// The `is_finite()` checks are in case the user is dragging the edge where the pivot is located (in which case the centering mode is ignored).
			if self.top {
				let ratio = (center_around.y - min.y) / (center_around.y - self.bounds[0].y);
				if ratio.is_finite() {
					max.y = center_around.y + ratio * (self.bounds[1].y - center_around.y);
					pivot.y = center_around.y;
				}
			} else if self.bottom {
				let ratio = (max.y - center_around.y) / (self.bounds[1].y - center_around.y);
				if ratio.is_finite() {
					min.y = center_around.y - ratio * (center_around.y - self.bounds[0].y);
					pivot.y = center_around.y;
				}
			}
			if self.left {
				let ratio = (center_around.x - min.x) / (center_around.x - self.bounds[0].x);
				if ratio.is_finite() {
					max.x = center_around.x + ratio * (self.bounds[1].x - center_around.x);
					pivot.x = center_around.x;
				}
			} else if self.right {
				let ratio = (max.x - center_around.x) / (self.bounds[1].x - center_around.x);
				if ratio.is_finite() {
					min.x = center_around.x - ratio * (center_around.x - self.bounds[0].x);
					pivot.x = center_around.x;
				}
			}
		}

		if constrain {
			let size = max - min;
			let min_pivot = (pivot - min) / size;
			let new_size = match ((self.top || self.bottom), (self.left || self.right)) {
				(true, true) => DVec2::new(size.x, size.x / self.aspect_ratio).abs().max(DVec2::new(size.y * self.aspect_ratio, size.y).abs()) * size.signum(),
				(true, false) => DVec2::new(size.y * self.aspect_ratio, size.y),
				(false, true) => DVec2::new(size.x, size.x / self.aspect_ratio),
				_ => size,
			};
			let delta_size = new_size - size;
			min -= delta_size * min_pivot;
			max = min + new_size;
		}

		(min, max - min)
	}

	/// Calculates the required scaling to resize the bounding box
	pub fn bounds_to_scale_transform(&self, position: DVec2, size: DVec2) -> (DAffine2, DVec2) {
		let old_size = self.bounds[1] - self.bounds[0];
		let mut enlargement_factor = size / old_size;
		if !enlargement_factor.x.is_finite() || old_size.x.abs() < f64::EPSILON * 1000. {
			enlargement_factor.x = 1.;
		}
		if !enlargement_factor.y.is_finite() || old_size.y.abs() < f64::EPSILON * 1000. {
			enlargement_factor.y = 1.;
		}
		let mut pivot = (self.bounds[0] * enlargement_factor - position) / (enlargement_factor - DVec2::splat(1.));
		if !pivot.x.is_finite() {
			pivot.x = 0.;
		}
		if !pivot.y.is_finite() {
			pivot.y = 0.;
		}
		(DAffine2::from_scale(enlargement_factor), pivot)
	}
}

/// Aligns the mouse position to the closest axis
pub fn axis_align_drag(axis_align: bool, position: DVec2, start: DVec2) -> DVec2 {
	if axis_align {
		let mouse_position = position - start;
		let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
		let angle = -mouse_position.angle_between(DVec2::X);
		let snapped_angle = (angle / snap_resolution).round() * snap_resolution;
		DVec2::new(snapped_angle.cos(), snapped_angle.sin()) * mouse_position.length() + start
	} else {
		position
	}
}

/// Contains info on the overlays for the bounding box and transform handles
#[derive(Clone, Debug, Default)]
pub struct BoundingBoxManager {
	pub bounds: [DVec2; 2],
	pub transform: DAffine2,
	pub original_bound_transform: DAffine2,
	pub selected_edges: Option<SelectedEdges>,
	pub original_transforms: OriginalTransforms,
	pub opposite_pivot: DVec2,
	pub center_of_transformation: DVec2,
}

impl BoundingBoxManager {
	/// Calculates the transformed handle positions based on the bounding box and the transform
	pub fn evaluate_transform_handle_positions(&self) -> [DVec2; 8] {
		let (left, top): (f64, f64) = self.bounds[0].into();
		let (right, bottom): (f64, f64) = self.bounds[1].into();
		[
			self.transform.transform_point2(DVec2::new(left, top)),
			self.transform.transform_point2(DVec2::new(left, (top + bottom) / 2.)),
			self.transform.transform_point2(DVec2::new(left, bottom)),
			self.transform.transform_point2(DVec2::new((left + right) / 2., top)),
			self.transform.transform_point2(DVec2::new((left + right) / 2., bottom)),
			self.transform.transform_point2(DVec2::new(right, top)),
			self.transform.transform_point2(DVec2::new(right, (top + bottom) / 2.)),
			self.transform.transform_point2(DVec2::new(right, bottom)),
		]
	}

	/// Update the position of the bounding box and transform handles
	pub fn render_overlays(&mut self, overlay_context: &mut OverlayContext) {
		overlay_context.quad(self.transform * Quad::from_box(self.bounds));

		for position in self.evaluate_transform_handle_positions() {
			overlay_context.square(position, false);
		}
	}

	/// Check if the user has selected the edge for dragging (returns which edge in order top, bottom, left, right)
	pub fn check_selected_edges(&self, cursor: DVec2) -> Option<(bool, bool, bool, bool)> {
		let cursor = self.transform.inverse().transform_point2(cursor);
		let select_threshold = self.transform.inverse().transform_vector2(DVec2::new(0., BOUNDS_SELECT_THRESHOLD)).length();

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);
		if min.x - cursor.x < select_threshold && min.y - cursor.y < select_threshold && cursor.x - max.x < select_threshold && cursor.y - max.y < select_threshold {
			let mut top = (cursor.y - min.y).abs() < select_threshold;
			let mut bottom = (max.y - cursor.y).abs() < select_threshold;
			let mut left = (cursor.x - min.x).abs() < select_threshold;
			let mut right = (max.x - cursor.x).abs() < select_threshold;

			// Prioritise single axis transformations on very small bounds
			if cursor.y - min.y + max.y - cursor.y < select_threshold * 2. && (left || right) {
				top = false;
				bottom = false;
			}
			if cursor.x - min.x + max.x - cursor.x < select_threshold * 2. && (top || bottom) {
				left = false;
				right = false;
			}

			// On bounds with no width/height, disallow transformation in the relevant axis
			if (max.x - min.x) < f64::EPSILON * 1000. {
				left = false;
				right = false;
			}
			if (max.y - min.y) < f64::EPSILON * 1000. {
				top = false;
				bottom = false;
			}

			if top || bottom || left || right {
				return Some((top, bottom, left, right));
			}
		}

		None
	}

	/// Check if the user is rotating with the bounds
	pub fn check_rotate(&self, cursor: DVec2) -> bool {
		let cursor = self.transform.inverse().transform_point2(cursor);
		let rotate_threshold = self.transform.inverse().transform_vector2(DVec2::new(0., BOUNDS_ROTATE_THRESHOLD)).length();

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);

		let outside_bounds = (min.x > cursor.x || cursor.x > max.x) || (min.y > cursor.y || cursor.y > max.y);
		let inside_extended_bounds = min.x - cursor.x < rotate_threshold && min.y - cursor.y < rotate_threshold && cursor.x - max.x < rotate_threshold && cursor.y - max.y < rotate_threshold;

		outside_bounds & inside_extended_bounds
	}

	/// Gets the required mouse cursor to show resizing bounds or optionally rotation
	pub fn get_cursor(&self, input: &InputPreprocessorMessageHandler, rotate: bool) -> MouseCursorIcon {
		if let Some(directions) = self.check_selected_edges(input.mouse.position) {
			match directions {
				(true, _, false, false) | (_, true, false, false) => MouseCursorIcon::NSResize,
				(false, false, true, _) | (false, false, _, true) => MouseCursorIcon::EWResize,
				(true, _, true, _) | (_, true, _, true) => MouseCursorIcon::NWSEResize,
				(true, _, _, true) | (_, true, true, _) => MouseCursorIcon::NESWResize,
				_ => MouseCursorIcon::Default,
			}
		} else if rotate && self.check_rotate(input.mouse.position) {
			MouseCursorIcon::Rotate
		} else {
			MouseCursorIcon::Default
		}
	}
}
