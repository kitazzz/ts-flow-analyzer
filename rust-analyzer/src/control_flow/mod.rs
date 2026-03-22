#[cfg(feature = "cfg-analysis")]
pub mod context;
#[cfg(feature = "cfg-analysis")]
pub mod reachability;
#[cfg(feature = "cfg-analysis")]
pub mod dot;

#[cfg(feature = "cfg-analysis")]
pub use context::build_cfg_context;
#[cfg(feature = "cfg-analysis")]
pub use dot::render_cfg_dot;
