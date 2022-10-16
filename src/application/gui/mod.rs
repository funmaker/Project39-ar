use std::collections::HashSet;
use egui::*;

mod selection;
mod main_tab;
mod physics_tab;

use super::{Application, Key};
pub use selection::GuiSelection;
use selection::GuiTab;
use main_tab::main_ui;
use physics_tab::physics_ui;

pub struct ApplicationGui {
	id: Id,
	tab: GuiTab,
	detached: HashSet<GuiTab>,
	grabbed: Option<GuiTab>,
	drag_pos: Pos2,
	closed: Vec<GuiTab>,
	panel_width_hint: f32,
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
		}
	}
	
	pub fn show(&mut self, ctx: &Context, application: &Application) {
		let tab_selection = application.gui_selection.borrow().tab();
		if !self.detached.contains(&tab_selection) {
			self.tab = tab_selection;
		}
		
		let openness = ctx.animate_bool(self.id, !application.input.keyboard.toggle(Key::Tab));
		if openness > 0.0 {
			SidePanel::right(self.id)
				.max_width(if openness >= 1.0 { f32::INFINITY } else { openness * self.panel_width_hint })
				.show(&ctx, |ui| {
					ui.horizontal(|ui| {
						self.show_tab_label(ui, GuiTab::Main, application);
						self.show_tab_label(ui, GuiTab::Physics, application);
						self.show_tab_label(ui, GuiTab::Benchmark, application);
						self.show_tab_label(ui, GuiTab::Settings, application);
						self.show_tab_label(ui, GuiTab::Inspector, application);
						self.show_tab_label(ui, GuiTab::Memory, application);
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
	}
	
	fn show_tab_label(&mut self, ui: &mut Ui, tab: GuiTab, application: &Application) {
		if self.detached.contains(&tab) {
			return;
		}
		
		let response = ui.selectable_value(&mut self.tab, tab, tab.label());
		
		if response.changed() {
			application.select(self.tab);
		}
		
		let response = response.interact(Sense::drag());
		
		if response.drag_delta().length_sq() > 16.0 && tab != GuiTab::Main {
			self.grabbed = Some(tab);
			if let Some(hover_pos) = ui.ctx().input().pointer.hover_pos() {
				self.drag_pos = hover_pos;
			}
		}
		
		if response.drag_released() && self.grabbed == Some(tab) {
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
		GuiTab::Physics => if let Ok(mut physics) = application.physics.try_borrow_mut() { physics_ui(&mut *physics, ui, application); },
		GuiTab::Benchmark => application.bench.borrow_mut().on_gui(ui),
		GuiTab::Settings => ctx.settings_ui(ui),
		GuiTab::Inspector => ctx.inspection_ui(ui),
		GuiTab::Memory => ctx.memory_ui(ui),
	}
}
