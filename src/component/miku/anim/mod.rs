
//! Humanoid animation system: clips imported from OpenViva, a Unity-like animator playing them and
//! retargeting of the resulting poses onto MMD models.

pub mod animator;
pub mod clip;
pub mod human;
pub mod pose;
pub mod retarget;

pub use animator::{Animator, ControllerDesc, LAYER_FACE, LAYER_LEGS, LAYER_TORSO};
pub use pose::Pose;
pub use retarget::{Retarget, SkeletonDesc};
