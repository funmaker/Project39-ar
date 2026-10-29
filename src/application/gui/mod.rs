use std::collections::HashSet;
use egui::*;

mod main_tab;
mod physics_tab;
mod selection;
mod miku_tab;

use crate::utils::ExUi;
use super::{Application, Key};
pub use selection::{GuiSelection, GuiSelections, select_button};
use main_tab::main_ui;
use physics_tab::physics_ui;
use selection::GuiTab;
use crate::application::gui::miku_tab::miku_ui;
use crate::application::window::StereoProjection;
use crate::debug;
use crate::math::{Color, Ray};

pub struct ApplicationGui {
	id: Id,
	tab: GuiTab,
	detached: HashSet<GuiTab>,
	grabbed: Option<GuiTab>,
	drag_pos: Pos2,
	closed: Vec<GuiTab>,
	panel_width_hint: f32,
	inspect: bool,
}

impl ApplicationGui {
	pub fn new() -> ApplicationGui {
		ApplicationGui {
			id: Id::new("SidePanel"),
			tab: GuiTab::Main,
			detached: HashSet::new(),
			grabbed: None,
			drag_pos: Pos2::ZERO,
			closed: vec![],
			panel_width_hint: f32::INFINITY,
			inspect: false,
		}
	}
	
	pub fn show(&mut self, ctx: &Context, application: &Application) {
		let gui_selections = application.gui_selections().clone();
		if gui_selections.focus {
			self.tab = gui_selections.selection.tab();
		}
		
		let openness = ctx.animate_bool(self.id, !application.input.keyboard.toggle(Key::Tab));
		if openness > 0.0 {
			SidePanel::right(self.id)
				.max_width(if openness >= 1.0 { f32::INFINITY } else { openness * self.panel_width_hint })
				.show(&ctx, |ui| {
					ui.horizontal(|ui| {
						ui.toggle_value(&mut self.inspect, "⯐");
							
						self.show_tab_label(ui, GuiTab::Main);
						self.show_tab_label(ui, GuiTab::Miku);
						self.show_tab_label(ui, GuiTab::Physics);
						self.show_tab_label(ui, GuiTab::Benchmark);
						self.show_tab_label(ui, GuiTab::Settings);
						self.show_tab_label(ui, GuiTab::Inspector);
						self.show_tab_label(ui, GuiTab::Memory);
					});
					ui.separator();
					
					if Some(self.tab) != self.grabbed {
						ScrollArea::vertical().show(ui, |ui| {
							show_tab(ui, self.tab, application);
						});
					}
					
					if openness >= 1.0 {
						self.panel_width_hint = ui.min_size().x;
					}
				});
		}
		
		for dtab in self.detached.iter().copied() {
			let mut open = true;
			
			Window::new(dtab.label())
				.vscroll(true)
				.open(&mut open)
				.show(ctx, |ui| show_tab(ui, dtab, application));
			
			if !open {
				self.closed.push(dtab);
			}
		}
		
		if let Some(dtab) = self.grabbed {
			Window::new(dtab.label())
				.current_pos(self.drag_pos)
				.show(ctx, |ui| show_tab(ui, dtab, application));
		}
		
		for closed in self.closed.drain(..) {
			self.detached.remove(&closed);
		}
		
		if self.inspect {
			CentralPanel::default().frame(Frame::NONE).show(&ctx, |ui| {
				let response = ui.interact(ui.clip_rect(), Id::new("Inspect"), Sense::click())
				                 .on_hover_cursor(CursorIcon::Crosshair);
				
				let hovered = ui.track_hover(&response);
				if response.hovered()
				&& let Some(inv_proj) = application.window.inv_proj
				&& let Some(cursor_pos) = ui.input(|input| input.pointer.hover_pos()) {
					fn to_dnc(pos: f32) -> f32 { pos * 2.0 - 1.0 }
					
					let screen_size = ui.ctx().screen_rect().size();
					let (proj, x, y) = match inv_proj {
						StereoProjection::Mono(proj) => (
							proj,
							to_dnc(cursor_pos.x / screen_size.x),
							to_dnc(cursor_pos.y / screen_size.y),
						),
						StereoProjection::Stereo(proj, _) if cursor_pos.x < screen_size.x / 2.0 => (
							proj,
							to_dnc(cursor_pos.x / screen_size.x * 2.0),
							to_dnc(cursor_pos.y / screen_size.y),
						),
						StereoProjection::Stereo(_, proj) => (
							proj,
							to_dnc(cursor_pos.x / screen_size.x * 2.0 - 1.0),
							to_dnc(cursor_pos.y / screen_size.y),
						),
					};
					
					let mut gui_selections = application.gui_selections_mut();
					let origin = proj.transform_point(&point!(x, y, -1.0));
					let target = proj.transform_point(&point!(x, y, 1.0));
					let ray = Ray::new(origin.into(), (target - origin).into());
					
					let hit_col = application.physics.borrow()
					                                 .query_pipeline()
					                                 .cast_ray(&ray, (origin - target).magnitude(), false)
					                                 .map_or(GuiSelection::None, |(col, _)| col.into());
					
					if response.clicked() {
						gui_selections.selection = hit_col.into();
						gui_selections.hover = GuiSelection::None;
						gui_selections.focus = gui_selections.selection != GuiSelection::None;
					} else {
						gui_selections.hover = hit_col.into();
					}
				} else if !response.hovered() && hovered {
					application.gui_selections_mut().hover = GuiSelection::None;
				}
				
				if response.clicked() {
					self.inspect = false;
				}
			});
		}
	}
	
	fn show_tab_label(&mut self, ui: &mut Ui, tab: GuiTab) {
		if self.detached.contains(&tab) {
			return;
		}
		
		let response = ui.selectable_value(&mut self.tab, tab, tab.label())
		                 .interact(Sense::drag());
		
		if response.drag_delta().length_sq() > 16.0 && tab != GuiTab::Main {
			self.grabbed = Some(tab);
			if let Some(hover_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
				self.drag_pos = hover_pos;
			}
		}
		
		if response.drag_stopped() && self.grabbed == Some(tab) {
			self.detached.insert(tab);
			self.grabbed = None;
			if self.tab == tab {
				self.tab = GuiTab::Main;
			}
		}
	}
}

fn show_tab(ui: &mut Ui, tab: GuiTab, application: &Application) {
	let ctx = ui.ctx().clone();
	
	match tab {
		GuiTab::Main => main_ui(ui, application),
		GuiTab::Miku => {
			let miku_sel = application.gui_selections()
			                          .selection
			                          .mmd_model()
			                          .entity();
			if let Some(miku) = miku_sel.get(application)
			                            .and_then(|miku| miku.find_component_by_type())
			                            .or_else(|| application.miku.get(application)) {
				miku_ui(miku, ui, application);
			}
		},
		GuiTab::Physics => if let Ok(mut physics) = application.physics.try_borrow_mut() { physics_ui(&mut *physics, ui, application); },
		GuiTab::Benchmark => application.bench.borrow_mut().on_gui(ui),
		GuiTab::Settings => ctx.settings_ui(ui),
		GuiTab::Inspector => ctx.inspection_ui(ui),
		GuiTab::Memory => ctx.memory_ui(ui),
	}
}
