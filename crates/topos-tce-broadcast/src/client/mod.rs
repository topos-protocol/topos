#[cfg(feature = "direct")]
pub mod direct;

#[cfg(not(feature = "direct"))]
pub mod reliable;
