use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::Entry;
use std::iter::FromIterator;
use std::time::Instant;
use egui::Ui;

const FONT_SIZE: f32 = 16.0;

pub struct Benchmark {
	samples: HashMap<&'static str, VecDeque<(f64, f64)>>,
	frame_start: Instant,
	current: Vec<(&'static str, Instant)>,
	order: Vec<&'static str>,
	frame_count: usize,
	paused: bool,
	single_pass: bool,
	hovered: Option<&'static str>,
	selected: Option<&'static str>,
}

impl Benchmark {
	pub fn new() -> Self {
		Self {
			samples: HashMap::new(),
			frame_start: Instant::now(),
			current: Vec::new(),
			order: Vec::new(),
			frame_count: 1000,
			paused: false,
			single_pass: false,
			hovered: None,
			selected: None,
		}
	}
	
	pub fn new_frame(&mut self) {
		if self.single_pass && self.samples.values().any(|queue| queue.len() == self.frame_count) {
			self.single_pass = false;
			self.paused = true;
		}
		
		if self.paused {
			self.frame_start = Instant::now();
			self.current.clear();
			return;
		}
		
		for queue in self.samples.values_mut() {
			queue.truncate(self.frame_count - 1);
			queue.push_front((0.0, 0.0));
		}
		
		let mut prev = self.frame_start;
		for &(name, time) in self.current.iter() {
			let entry = self.samples.entry(name);
			
			let queue = match entry {
				Entry::Occupied(entry) => entry.into_mut(),
				Entry::Vacant(entry) => {
					self.order.push(name);
					entry.insert(VecDeque::with_capacity(self.frame_count))
				}
			};
			
			let rel = (time - prev).as_secs_f64() * 1000.0;
			let abs = (time - self.frame_start).as_secs_f64() * 1000.0;
			prev = time;
			
			queue.pop_front();
			queue.push_front((abs, rel));
		}
		
		let current = self.current.drain(..)
		                          .collect::<Vec<_>>();
		
		self.order.sort_by_key(|name| current.iter()
		                                     .find(|(n, _)| n == name)
		                                     .map(|(_, i)| *i));
		
		self.frame_start = Instant::now();
	}
	
	pub fn tick(&mut self, stage: &'static str) {
		self.current.push((stage, Instant::now()))
	}
	
	pub fn on_gui(&mut self, ui: &mut Ui) {
		use egui::plot::*;
		use egui::*;
		const COLORS: [Color32; 8] = [
			Color32::from_rgb(255, 0, 0),
			Color32::from_rgb(191, 143, 0),
			Color32::from_rgb(128, 255, 0),
			Color32::from_rgb(0, 191, 48),
			Color32::from_rgb(0, 255, 255),
			Color32::from_rgb(64, 64, 255),
			Color32::from_rgb(128, 0, 255),
			Color32::from_rgb(191, 0, 143),
		];
		
		ui.allocate_space([ui.available_rect_before_wrap().width(), 0.0].into());
		
		let name_len = self.order.iter()
		                   .map(|name| name.len())
		                   .max()
		                   .unwrap_or(0);
		let col_width = (name_len + 35) as f32 * FONT_SIZE * 0.5;
		let cols = (ui.min_size().x / col_width).max(1.0) as usize;
		let rows = self.order.len().div_ceil(cols);
		
		ui.set_min_width(col_width);
		
		Plot::new("Benchmark Plot")
			.view_aspect(2.0)
			.include_y(0.0)
			.include_x(0.0)
			.include_x(-(self.frame_count as f64) + 1.0)
			.show(ui, |plot_ui| {
				for (i, &name) in self.order.iter().enumerate() {
					let color = if self.selected.is_none() && self.hovered.is_none() {
						COLORS[i % COLORS.len()]
					} else {
						Color32::GRAY
					};
					
					plot_ui.line(Line::new(PlotPoints::from_iter(
						self.samples[name].iter()
						     .copied()
						     .enumerate()
						     .map(|(i, (abs, _))| [-(i as f64), abs])
					)).color(color));
				}
				
				if let Some(name) = self.selected.or(self.hovered) {
					let color = self.order.iter()
					                      .copied()
					                      .enumerate()
					                      .find(|&(_, n)| n == name)
					                      .map_or(Color32::WHITE, |(i, _)| COLORS[i % COLORS.len()]);
					
					plot_ui.line(Line::new(PlotPoints::from_iter(
						self.samples[name].iter()
						                  .copied()
						                  .enumerate()
						                  .map(|(i, (abs, _))| [-(i as f64), abs])
					)).color(color)
					  .highlight(true));
					
					plot_ui.bar_chart(BarChart::new(
						self.samples[name].iter()
						                  .copied()
						                  .enumerate()
						                  .map(|(i, (_, rel))| Bar::new(-(i as f64), rel))
						                  .collect()
					).width(1.0)
					 .color(color));
				}
				
				plot_ui.hline(HLine::new(1000.0 / 144.0).color(Color32::RED))
			});
		
		self.hovered = None;
		
		ui.horizontal(|ui| {
			for col in 0..cols {
				ui.vertical(|ui| {
					ui.label(
						RichText::new(format!("  {:width$}     cur     min     max     avr", "stage", width = name_len))
							.size(16.0)
							.monospace()
					);
					
					for (i, &name) in self.order.iter().enumerate().skip(col * rows).take(rows) {
						let queue = &self.samples[name];
						
						let cur = queue.get(0).map(|(_, rel)| *rel).unwrap_or_default();
						let min = queue.iter().map(|(_, rel)| *rel).reduce(f64::min).unwrap_or_default();
						let max = queue.iter().map(|(_, rel)| *rel).reduce(f64::max).unwrap_or_default();
						let avr = queue.iter().map(|(_, rel)| *rel).sum::<f64>() / queue.len() as f64;
						
						
						let color = if self.selected.map_or(true, |n| n == name) {
							COLORS[i % COLORS.len()]
						} else {
							Color32::GRAY
						};
						
						let response = Label::new(
							RichText::new(format!("- {:width$} {:>7.3} {:>7.3} {:>7.3} {:>7.3}", name, cur, min, max, avr, width = name_len))
								.color(color)
								.size(FONT_SIZE)
								.monospace())
							.sense(Sense::click())
							.ui(ui);
						
						if response.clicked() {
							if self.selected == Some(name) {
								self.selected = None;
							} else {
								self.selected = Some(name);
							}
						}
						
						if response.hovered() {
							self.hovered = Some(name);
						}
					}
				});
			}
		});
		
		ui.separator();
		
		ui.horizontal_wrapped(|ui| {
			if self.paused {
				if ui.button(RichText::new("Resume").color(Color32::BLUE)).clicked() {
					self.paused = false;
				}
			} else {
				if ui.button("Pause").clicked() {
					self.paused = true;
				}
			}
			
			if self.single_pass {
				if ui.button(RichText::new("Single Pass").color(Color32::BLUE)).clicked() {
					self.single_pass = false;
				}
			} else {
				if ui.button("Single Pass").clicked() {
					self.samples.values_mut().for_each(VecDeque::clear);
					self.single_pass = true;
					self.paused = false;
				}
			}
			
			if ui.button("Clear").clicked() {
				self.samples.values_mut().for_each(VecDeque::clear);
			}
			
			ui.label("Buffer Size: ");
			
			Slider::new(&mut self.frame_count, 2..=10000)
				.logarithmic(true)
				.ui(ui)
		});
	}
}
