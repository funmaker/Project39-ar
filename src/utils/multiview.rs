// Rewrite from render_pass/render_pass.rs

// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use vulkano::device::Device;
use vulkano::device::DeviceOwned;
use vulkano::format::FormatTy;
use vulkano::image::ImageLayout;
use vulkano::pipeline::shader::ShaderInterfaceDef;
use vulkano::render_pass::AttachmentDesc;
use vulkano::render_pass::LoadOp;
use vulkano::render_pass::RenderPassDesc;
use vulkano::render_pass::SubpassDesc;
use vulkano::OomError;
use vulkano::VulkanObject;
use vk_sys as vk;
use smallvec::SmallVec;
use std::error;
use std::fmt;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::ffi::c_void;

/// An object representing the discrete steps in which rendering is done.
///
/// A render pass in Vulkan is made up of three parts:
/// - A list of attachments, which are image views that are inputs, outputs or intermediate stages
///   in the rendering process.
/// - One or more subpasses, which are the steps in which the rendering process, takes place,
///   and the attachments that are used for each step.
/// - Dependencies, which describe how the input and output data of each subpass is to be passed
///   from one subpass to the next.
///
/// In order to create a render pass, you must create a `RenderPassDesc` object that describes the
/// render pass, then pass it to `RenderPass::new`.
///
/// ```
/// use vulkano::render_pass::RenderPass;
/// use vulkano::render_pass::RenderPassDesc;
///
/// # let device: std::sync::Arc<vulkano::device::Device> = return;
/// let desc = RenderPassDesc::empty();
/// let render_pass = RenderPass::new(device.clone(), desc).unwrap();
/// ```
///
/// This example creates a render pass with no attachment and one single subpass that doesn't draw
/// on anything. While it's sometimes useful, most of the time it's not what you want.
///
/// The easiest way to create a "real" render pass is to use the `single_pass_renderpass!` macro.
///
/// ```
/// # #[macro_use] extern crate vulkano;
/// # fn main() {
/// # let device: std::sync::Arc<vulkano::device::Device> = return;
/// use vulkano::format::Format;
///
/// let render_pass = single_pass_renderpass!(device.clone(),
///     attachments: {
///         // `foo` is a custom name we give to the first and only attachment.
///         foo: {
///             load: Clear,
///             store: Store,
///             format: Format::R8G8B8A8Unorm,
///             samples: 1,
///         }
///     },
///     pass: {
///         color: [foo],       // Repeat the attachment name here.
///         depth_stencil: {}
///     }
/// ).unwrap();
/// # }
/// ```
///
/// See the documentation of the macro for more details. TODO: put link here
pub struct RenderPass {
	// The internal Vulkan object.
	render_pass: vk::RenderPass,
	
	// Device this render pass was created from.
	device: Arc<Device>,
	
	// Description of the render pass.
	desc: RenderPassDesc,
	
	// Cache of the granularity of the render pass.
	granularity: Mutex<Option<[u32; 2]>>,
}

impl RenderPass {
	/// Builds a new render pass.
	///
	/// # Panic
	///
	/// - Can panic if it detects some violations in the restrictions. Only inexpensive checks are
	/// performed. `debug_assert!` is used, so some restrictions are only checked in debug
	/// mode.
	///
	pub fn new(
		device: Arc<Device>,
		description: RenderPassDesc,
		views: usize,
	) -> Result<RenderPass, RenderPassCreationError> {
		let vk = device.pointers();
		
		// If the first use of an attachment in this render pass is as an input attachment, and
		// the attachment is not also used as a color or depth/stencil attachment in the same
		// subpass, then loadOp must not be VK_ATTACHMENT_LOAD_OP_CLEAR
		debug_assert!(description.attachments().into_iter().enumerate().all(
			|(atch_num, attachment)| {
				if attachment.load != LoadOp::Clear {
					return true;
				}
				
				for p in description.subpasses() {
					if p.color_attachments
					    .iter()
					    .find(|&&(a, _)| a == atch_num)
					    .is_some()
					{
						return true;
					}
					if let Some((a, _)) = p.depth_stencil {
						if a == atch_num {
							return true;
						}
					}
					if p.input_attachments
					    .iter()
					    .find(|&&(a, _)| a == atch_num)
					    .is_some()
					{
						return false;
					}
				}
				
				true
			}
		));
		
		let attachments = description
			.attachments()
			.iter()
			.map(|attachment| {
				debug_assert!(attachment.samples.is_power_of_two());
				
				vk::AttachmentDescription {
					flags: 0, // FIXME: may alias flag
					format: attachment.format as u32,
					samples: attachment.samples,
					loadOp: attachment.load as u32,
					storeOp: attachment.store as u32,
					stencilLoadOp: attachment.stencil_load as u32,
					stencilStoreOp: attachment.stencil_store as u32,
					initialLayout: attachment.initial_layout as u32,
					finalLayout: attachment.final_layout as u32,
				}
			})
			.collect::<SmallVec<[_; 16]>>();
		
		// We need to pass pointers to vkAttachmentReference structs when creating the render pass.
		// Therefore we need to allocate them in advance.
		//
		// This block allocates, for each pass, in order, all color attachment references, then all
		// input attachment references, then all resolve attachment references, then the depth
		// stencil attachment reference.
		let attachment_references = description
			.subpasses()
			.iter()
			.flat_map(|pass| {
				// Performing some validation with debug asserts.
				debug_assert!(
					pass.resolve_attachments.is_empty()
						|| pass.resolve_attachments.len() == pass.color_attachments.len()
				);
				debug_assert!(pass
					.resolve_attachments
					.iter()
					.all(|a| attachments[a.0].samples == 1));
				debug_assert!(
					pass.resolve_attachments.is_empty()
						|| pass
						.color_attachments
						.iter()
						.all(|a| attachments[a.0].samples > 1)
				);
				debug_assert!(
					pass.resolve_attachments.is_empty()
						|| pass
						.resolve_attachments
						.iter()
						.zip(pass.color_attachments.iter())
						.all(|(r, c)| { attachments[r.0].format == attachments[c.0].format })
				);
				debug_assert!(pass
					.color_attachments
					.iter()
					.cloned()
					.chain(pass.depth_stencil.clone().into_iter())
					.chain(pass.input_attachments.iter().cloned())
					.chain(pass.resolve_attachments.iter().cloned())
					.all(|(a, _)| {
						pass.preserve_attachments
						    .iter()
						    .find(|&&b| a == b)
						    .is_none()
					}));
				debug_assert!(pass
					.color_attachments
					.iter()
					.cloned()
					.chain(pass.depth_stencil.clone().into_iter())
					.all(|(atch, layout)| {
						if let Some(r) = pass.input_attachments.iter().find(|r| r.0 == atch) {
							r.1 == layout
						} else {
							true
						}
					}));
				
				let resolve = pass.resolve_attachments.iter().map(|&(offset, img_la)| {
					debug_assert!(offset < attachments.len());
					vk::AttachmentReference {
						attachment: offset as u32,
						layout: img_la as u32,
					}
				});
				
				let color = pass.color_attachments.iter().map(|&(offset, img_la)| {
					debug_assert!(offset < attachments.len());
					vk::AttachmentReference {
						attachment: offset as u32,
						layout: img_la as u32,
					}
				});
				
				let input = pass.input_attachments.iter().map(|&(offset, img_la)| {
					debug_assert!(offset < attachments.len());
					vk::AttachmentReference {
						attachment: offset as u32,
						layout: img_la as u32,
					}
				});
				
				let depthstencil = if let Some((offset, img_la)) = pass.depth_stencil {
					Some(vk::AttachmentReference {
						attachment: offset as u32,
						layout: img_la as u32,
					})
				} else {
					None
				}
					.into_iter();
				
				color.chain(input).chain(resolve).chain(depthstencil)
			})
			.collect::<SmallVec<[_; 16]>>();
		
		// Same as `attachment_references` but only for the preserve attachments.
		// This is separate because attachment references are u32s and not `vkAttachmentReference`
		// structs.
		let preserve_attachments_references = description
			.subpasses()
			.iter()
			.flat_map(|pass| {
				pass.preserve_attachments
				    .iter()
				    .map(|&offset| offset as u32)
			})
			.collect::<SmallVec<[_; 16]>>();
		
		// Now iterating over passes.
		let passes = unsafe {
			// `ref_index` and `preserve_ref_index` are increased during the loop and point to the
			// next element to use in respectively `attachment_references` and
			// `preserve_attachments_references`.
			let mut ref_index = 0usize;
			let mut preserve_ref_index = 0usize;
			let mut out: SmallVec<[_; 16]> = SmallVec::new();
			
			for pass in description.subpasses() {
				if pass.color_attachments.len() as u32
					> device.physical_device().limits().max_color_attachments()
				{
					return Err(RenderPassCreationError::ColorAttachmentsLimitExceeded);
				}
				
				let color_attachments = attachment_references.as_ptr().offset(ref_index as isize);
				ref_index += pass.color_attachments.len();
				let input_attachments = attachment_references.as_ptr().offset(ref_index as isize);
				ref_index += pass.input_attachments.len();
				let resolve_attachments = attachment_references.as_ptr().offset(ref_index as isize);
				ref_index += pass.resolve_attachments.len();
				let depth_stencil = if pass.depth_stencil.is_some() {
					let a = attachment_references.as_ptr().offset(ref_index as isize);
					ref_index += 1;
					a
				} else {
					ptr::null()
				};
				
				let preserve_attachments = preserve_attachments_references
					.as_ptr()
					.offset(preserve_ref_index as isize);
				preserve_ref_index += pass.preserve_attachments.len();
				
				out.push(vk::SubpassDescription {
					flags: 0, // reserved
					pipelineBindPoint: vk::PIPELINE_BIND_POINT_GRAPHICS,
					inputAttachmentCount: pass.input_attachments.len() as u32,
					pInputAttachments: if pass.input_attachments.is_empty() {
						ptr::null()
					} else {
						input_attachments
					},
					colorAttachmentCount: pass.color_attachments.len() as u32,
					pColorAttachments: if pass.color_attachments.is_empty() {
						ptr::null()
					} else {
						color_attachments
					},
					pResolveAttachments: if pass.resolve_attachments.is_empty() {
						ptr::null()
					} else {
						resolve_attachments
					},
					pDepthStencilAttachment: depth_stencil,
					preserveAttachmentCount: pass.preserve_attachments.len() as u32,
					pPreserveAttachments: if pass.preserve_attachments.is_empty() {
						ptr::null()
					} else {
						preserve_attachments
					},
				});
			}
			
			assert!(!out.is_empty());
			// If these assertions fails, there's a serious bug in the code above ^.
			debug_assert!(ref_index == attachment_references.len());
			debug_assert!(preserve_ref_index == preserve_attachments_references.len());
			
			out
		};
		
		let dependencies = description
			.dependencies()
			.iter()
			.map(|dependency| {
				debug_assert!(
					dependency.source_subpass as u32 == vk::SUBPASS_EXTERNAL
						|| dependency.source_subpass < passes.len()
				);
				debug_assert!(
					dependency.destination_subpass as u32 == vk::SUBPASS_EXTERNAL
						|| dependency.destination_subpass < passes.len()
				);
				
				vk::SubpassDependency {
					srcSubpass: dependency.source_subpass as u32,
					dstSubpass: dependency.destination_subpass as u32,
					srcStageMask: dependency.source_stages.into(),
					dstStageMask: dependency.destination_stages.into(),
					srcAccessMask: dependency.source_access.into(),
					dstAccessMask: dependency.destination_access.into(),
					dependencyFlags: if dependency.by_region {
						vk::DEPENDENCY_BY_REGION_BIT
					} else {
						0
					},
				}
			})
			.collect::<SmallVec<[_; 16]>>();
		
		let view_mask = (1 << views) - 1;
		let view_masks = passes.iter().map(|_| view_mask).collect::<SmallVec<[_; 16]>>();
		
		let multiview_ext = RenderPassMultiviewCreateInfo {
			sType: vk::STRUCTURE_TYPE_RENDER_PASS_MULTIVIEW_CREATE_INFO,
			pNext: ptr::null(),
			subpassCount: passes.len(),
			pViewMasks: view_masks.as_ptr(),
			dependencyCount: 0,
			pViewOffsets: ptr::null(),
			correlationMaskCount: 0,
			pDependencies: ptr::null()
		};
		
		let render_pass = unsafe {
			let infos = vk::RenderPassCreateInfo {
				sType: vk::STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
				pNext: &multiview_ext as *const _,
				flags: 0, // reserved
				attachmentCount: attachments.len() as u32,
				pAttachments: if attachments.is_empty() {
					ptr::null()
				} else {
					attachments.as_ptr()
				},
				subpassCount: passes.len() as u32,
				pSubpasses: if passes.is_empty() {
					ptr::null()
				} else {
					passes.as_ptr()
				},
				dependencyCount: dependencies.len() as u32,
				pDependencies: if dependencies.is_empty() {
					ptr::null()
				} else {
					dependencies.as_ptr()
				},
			};
			
			let mut output = MaybeUninit::uninit();
			check_errors(vk.CreateRenderPass(
				device.internal_object(),
				&infos,
				ptr::null(),
				output.as_mut_ptr(),
			))?;
			output.assume_init()
		};
		
		Ok(RenderPass {
			device: device.clone(),
			render_pass,
			desc: description,
			granularity: Mutex::new(None),
		})
	}
	
	/// Builds a render pass with one subpass and no attachment.
	///
	/// This method is useful for quick tests.
	#[inline]
	pub fn empty_single_pass(device: Arc<Device>) -> Result<RenderPass, RenderPassCreationError> {
		RenderPass::new(device, RenderPassDesc::empty(), 0)
	}
	
	#[inline]
	pub fn inner(&self) -> RenderPassSys {
		RenderPassSys(self.render_pass, PhantomData)
	}
	
	/// Returns the granularity of this render pass.
	///
	/// If the render area of a render pass in a command buffer is a multiple of this granularity,
	/// then the performance will be optimal. Performances are always optimal for render areas
	/// that cover the whole framebuffer.
	pub fn granularity(&self) -> [u32; 2] {
		let mut granularity = self.granularity.lock().unwrap();
		
		if let Some(&granularity) = granularity.as_ref() {
			return granularity;
		}
		
		unsafe {
			let vk = self.device.pointers();
			let mut out = MaybeUninit::uninit();
			vk.GetRenderAreaGranularity(
				self.device.internal_object(),
				self.render_pass,
				out.as_mut_ptr(),
			);
			
			let out = out.assume_init();
			debug_assert_ne!(out.width, 0);
			debug_assert_ne!(out.height, 0);
			let gran = [out.width, out.height];
			*granularity = Some(gran);
			gran
		}
	}
	
	/// Returns the description of the render pass.
	#[inline]
	pub fn desc(&self) -> &RenderPassDesc {
		&self.desc
	}
}

unsafe impl DeviceOwned for RenderPass {
	#[inline]
	fn device(&self) -> &Arc<Device> {
		&self.device
	}
}

impl fmt::Debug for RenderPass {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		fmt.debug_struct("RenderPass")
		   .field("raw", &self.render_pass)
		   .field("device", &self.device)
		   .field("desc", &self.desc)
		   .finish()
	}
}

impl Drop for RenderPass {
	#[inline]
	fn drop(&mut self) {
		unsafe {
			let vk = self.device.pointers();
			vk.DestroyRenderPass(self.device.internal_object(), self.render_pass, ptr::null());
		}
	}
}

/// Opaque object that represents the render pass' internals.
#[derive(Debug, Copy, Clone)]
pub struct RenderPassSys<'a>(vk::RenderPass, PhantomData<&'a ()>);

unsafe impl<'a> VulkanObject for RenderPassSys<'a> {
	type Object = vk::RenderPass;
	
	const TYPE: vk::ObjectType = vk::OBJECT_TYPE_RENDER_PASS;
	
	#[inline]
	fn internal_object(&self) -> vk::RenderPass {
		self.0
	}
}

/// Error that can happen when creating a compute pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderPassCreationError {
	/// Not enough memory.
	OomError(OomError),
	/// The maximum number of color attachments has been exceeded.
	ColorAttachmentsLimitExceeded,
}

impl error::Error for RenderPassCreationError {
	#[inline]
	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match *self {
			RenderPassCreationError::OomError(ref err) => Some(err),
			_ => None,
		}
	}
}

impl fmt::Display for RenderPassCreationError {
	#[inline]
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		write!(
			fmt,
			"{}",
			match *self {
				RenderPassCreationError::OomError(_) => "not enough memory available",
				RenderPassCreationError::ColorAttachmentsLimitExceeded => {
					"the maximum number of color attachments has been exceeded"
				}
			}
		)
	}
}

impl From<OomError> for RenderPassCreationError {
	#[inline]
	fn from(err: OomError) -> RenderPassCreationError {
		RenderPassCreationError::OomError(err)
	}
}

impl From<Error> for RenderPassCreationError {
	#[inline]
	fn from(err: Error) -> RenderPassCreationError {
		match err {
			err @ Error::OutOfHostMemory => RenderPassCreationError::OomError(OomError::from(err)),
			err @ Error::OutOfDeviceMemory => {
				RenderPassCreationError::OomError(OomError::from(err))
			}
			_ => panic!("unexpected error: {:?}", err),
		}
	}
}

/// Represents a subpass within a `RenderPass` object.
///
/// This struct doesn't correspond to anything in Vulkan. It is simply an equivalent to a
/// tuple of a render pass and subpass index. Contrary to a tuple, however, the existence of the
/// subpass is checked when the object is created. When you have a `Subpass` you are guaranteed
/// that the given subpass does exist.
#[derive(Debug, Clone)]
pub struct Subpass {
	render_pass: Arc<RenderPass>,
	subpass_id: u32,
}

impl Subpass {
	/// Returns a handle that represents a subpass of a render pass.
	#[inline]
	pub fn from(render_pass: Arc<RenderPass>, id: u32) -> Option<Subpass> {
		if (id as usize) < render_pass.desc().subpasses().len() {
			Some(Subpass {
				render_pass,
				subpass_id: id,
			})
		} else {
			None
		}
	}
	
	#[inline]
	fn subpass_desc(&self) -> &SubpassDesc {
		&self.render_pass.desc().subpasses()[self.subpass_id as usize]
	}
	
	#[inline]
	fn attachment_desc(&self, atch_num: usize) -> &AttachmentDesc {
		&self.render_pass.desc().attachments()[atch_num]
	}
	
	/// Returns the number of color attachments in this subpass.
	#[inline]
	pub fn num_color_attachments(&self) -> u32 {
		self.subpass_desc().color_attachments.len() as u32
	}
	
	/// Returns true if the subpass has a depth attachment or a depth-stencil attachment.
	#[inline]
	pub fn has_depth(&self) -> bool {
		let subpass_desc = self.subpass_desc();
		let atch_num = match subpass_desc.depth_stencil {
			Some((d, _)) => d,
			None => return false,
		};
		
		match self.attachment_desc(atch_num).format.ty() {
			FormatTy::Depth => true,
			FormatTy::Stencil => false,
			FormatTy::DepthStencil => true,
			_ => unreachable!(),
		}
	}
	
	/// Returns true if the subpass has a depth attachment or a depth-stencil attachment whose
	/// layout is not `DepthStencilReadOnlyOptimal`.
	#[inline]
	pub fn has_writable_depth(&self) -> bool {
		let subpass_desc = self.subpass_desc();
		let atch_num = match subpass_desc.depth_stencil {
			Some((d, l)) => {
				if l == ImageLayout::DepthStencilReadOnlyOptimal {
					return false;
				}
				d
			}
			None => return false,
		};
		
		match self.attachment_desc(atch_num).format.ty() {
			FormatTy::Depth => true,
			FormatTy::Stencil => false,
			FormatTy::DepthStencil => true,
			_ => unreachable!(),
		}
	}
	
	/// Returns true if the subpass has a stencil attachment or a depth-stencil attachment.
	#[inline]
	pub fn has_stencil(&self) -> bool {
		let subpass_desc = self.subpass_desc();
		let atch_num = match subpass_desc.depth_stencil {
			Some((d, _)) => d,
			None => return false,
		};
		
		match self.attachment_desc(atch_num).format.ty() {
			FormatTy::Depth => false,
			FormatTy::Stencil => true,
			FormatTy::DepthStencil => true,
			_ => unreachable!(),
		}
	}
	
	/// Returns true if the subpass has a stencil attachment or a depth-stencil attachment whose
	/// layout is not `DepthStencilReadOnlyOptimal`.
	#[inline]
	pub fn has_writable_stencil(&self) -> bool {
		let subpass_desc = self.subpass_desc();
		
		let atch_num = match subpass_desc.depth_stencil {
			Some((d, l)) => {
				if l == ImageLayout::DepthStencilReadOnlyOptimal {
					return false;
				}
				d
			}
			None => return false,
		};
		
		match self.attachment_desc(atch_num).format.ty() {
			FormatTy::Depth => false,
			FormatTy::Stencil => true,
			FormatTy::DepthStencil => true,
			_ => unreachable!(),
		}
	}
	
	/// Returns true if the subpass has any color or depth/stencil attachment.
	#[inline]
	pub fn has_color_or_depth_stencil_attachment(&self) -> bool {
		if self.num_color_attachments() >= 1 {
			return true;
		}
		
		let subpass_desc = self.subpass_desc();
		match subpass_desc.depth_stencil {
			Some((d, _)) => true,
			None => false,
		}
	}
	
	/// Returns the number of samples in the color and/or depth/stencil attachments. Returns `None`
	/// if there is no such attachment in this subpass.
	#[inline]
	pub fn num_samples(&self) -> Option<u32> {
		let subpass_desc = self.subpass_desc();
		
		// TODO: chain input attachments as well?
		subpass_desc
			.color_attachments
			.iter()
			.cloned()
			.chain(subpass_desc.depth_stencil.clone().into_iter())
			.filter_map(|a| self.render_pass.desc().attachments().get(a.0))
			.next()
			.map(|a| a.samples)
	}
	
	/// Returns the render pass of this subpass.
	#[inline]
	pub fn render_pass(&self) -> &Arc<RenderPass> {
		&self.render_pass
	}
	
	/// Returns the index of this subpass within the renderpass.
	#[inline]
	pub fn index(&self) -> u32 {
		self.subpass_id
	}
	
	/// Returns `true` if this subpass is compatible with the fragment output definition.
	// TODO: return proper error
	pub fn is_compatible_with<S>(&self, shader_interface: &S) -> bool
		where
			S: ShaderInterfaceDef,
	{
		self.render_pass
		    .desc()
		    .is_compatible_with_shader(self.subpass_id, shader_interface)
	}
}

impl From<Subpass> for (Arc<RenderPass>, u32) {
	#[inline]
	fn from(value: Subpass) -> (Arc<RenderPass>, u32) {
		(value.render_pass, value.subpass_id)
	}
}

#[allow(non_snake_case)]
#[repr(C)]
pub struct RenderPassMultiviewCreateInfo {
	pub sType: vk::StructureType,
	pub pNext: *const c_void,
	pub subpassCount: u32,
	pub pViewMasks: *const u32,
	pub dependencyCount: u32,
	pub pViewOffsets: *const i32,
	pub correlationMaskCount: u32,
	pub pDependencies: *const u32,
}


/// All possible success codes returned by any Vulkan function.
#[derive(Debug, Copy, Clone)]
#[repr(u32)]
enum Success {
	Success = vk::SUCCESS,
	NotReady = vk::NOT_READY,
	Timeout = vk::TIMEOUT,
	EventSet = vk::EVENT_SET,
	EventReset = vk::EVENT_RESET,
	Incomplete = vk::INCOMPLETE,
	Suboptimal = vk::SUBOPTIMAL_KHR,
}

/// All possible errors returned by any Vulkan function.
///
/// This type is not public. Instead all public error types should implement `From<Error>` and
/// panic for error code that aren't supposed to happen.
#[derive(Debug, Copy, Clone)]
#[repr(u32)]
// TODO: being pub is necessary because of the weird visibility rules in rustc
pub(crate) enum Error {
	OutOfHostMemory = vk::ERROR_OUT_OF_HOST_MEMORY,
	OutOfDeviceMemory = vk::ERROR_OUT_OF_DEVICE_MEMORY,
	InitializationFailed = vk::ERROR_INITIALIZATION_FAILED,
	DeviceLost = vk::ERROR_DEVICE_LOST,
	MemoryMapFailed = vk::ERROR_MEMORY_MAP_FAILED,
	LayerNotPresent = vk::ERROR_LAYER_NOT_PRESENT,
	ExtensionNotPresent = vk::ERROR_EXTENSION_NOT_PRESENT,
	FeatureNotPresent = vk::ERROR_FEATURE_NOT_PRESENT,
	IncompatibleDriver = vk::ERROR_INCOMPATIBLE_DRIVER,
	TooManyObjects = vk::ERROR_TOO_MANY_OBJECTS,
	FormatNotSupported = vk::ERROR_FORMAT_NOT_SUPPORTED,
	SurfaceLost = vk::ERROR_SURFACE_LOST_KHR,
	NativeWindowInUse = vk::ERROR_NATIVE_WINDOW_IN_USE_KHR,
	OutOfDate = vk::ERROR_OUT_OF_DATE_KHR,
	IncompatibleDisplay = vk::ERROR_INCOMPATIBLE_DISPLAY_KHR,
	ValidationFailed = vk::ERROR_VALIDATION_FAILED_EXT,
	OutOfPoolMemory = vk::ERROR_OUT_OF_POOL_MEMORY_KHR,
	FullscreenExclusiveLost = vk::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT,
}

/// Checks whether the result returned correctly.
fn check_errors(result: vk::Result) -> Result<Success, Error> {
	match result {
		vk::SUCCESS => Ok(Success::Success),
		vk::NOT_READY => Ok(Success::NotReady),
		vk::TIMEOUT => Ok(Success::Timeout),
		vk::EVENT_SET => Ok(Success::EventSet),
		vk::EVENT_RESET => Ok(Success::EventReset),
		vk::INCOMPLETE => Ok(Success::Incomplete),
		vk::ERROR_OUT_OF_HOST_MEMORY => Err(Error::OutOfHostMemory),
		vk::ERROR_OUT_OF_DEVICE_MEMORY => Err(Error::OutOfDeviceMemory),
		vk::ERROR_INITIALIZATION_FAILED => Err(Error::InitializationFailed),
		vk::ERROR_DEVICE_LOST => Err(Error::DeviceLost),
		vk::ERROR_MEMORY_MAP_FAILED => Err(Error::MemoryMapFailed),
		vk::ERROR_LAYER_NOT_PRESENT => Err(Error::LayerNotPresent),
		vk::ERROR_EXTENSION_NOT_PRESENT => Err(Error::ExtensionNotPresent),
		vk::ERROR_FEATURE_NOT_PRESENT => Err(Error::FeatureNotPresent),
		vk::ERROR_INCOMPATIBLE_DRIVER => Err(Error::IncompatibleDriver),
		vk::ERROR_TOO_MANY_OBJECTS => Err(Error::TooManyObjects),
		vk::ERROR_FORMAT_NOT_SUPPORTED => Err(Error::FormatNotSupported),
		vk::ERROR_SURFACE_LOST_KHR => Err(Error::SurfaceLost),
		vk::ERROR_NATIVE_WINDOW_IN_USE_KHR => Err(Error::NativeWindowInUse),
		vk::SUBOPTIMAL_KHR => Ok(Success::Suboptimal),
		vk::ERROR_OUT_OF_DATE_KHR => Err(Error::OutOfDate),
		vk::ERROR_INCOMPATIBLE_DISPLAY_KHR => Err(Error::IncompatibleDisplay),
		vk::ERROR_VALIDATION_FAILED_EXT => Err(Error::ValidationFailed),
		vk::ERROR_OUT_OF_POOL_MEMORY_KHR => Err(Error::OutOfPoolMemory),
		vk::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT => Err(Error::FullscreenExclusiveLost),
		vk::ERROR_INVALID_SHADER_NV => panic!(
			"Vulkan function returned \
                                               VK_ERROR_INVALID_SHADER_NV"
		),
		c => unreachable!("Unexpected error code returned by Vulkan: {}", c),
	}
}

impl From<Error> for OomError {
	#[inline]
	fn from(err: Error) -> OomError {
		match err {
			Error::OutOfHostMemory => OomError::OutOfHostMemory,
			Error::OutOfDeviceMemory => OomError::OutOfDeviceMemory,
			_ => panic!("unexpected error: {:?}", err),
		}
	}
}
