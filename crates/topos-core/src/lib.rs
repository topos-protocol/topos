#[cfg_attr(docsrs, doc(cfg(feature = "uci")))]
pub mod uci;

#[cfg_attr(docsrs, doc(cfg(feature = "api")))]
pub mod api;

pub mod errors;
pub mod types;

#[cfg(test)]
mod test;
