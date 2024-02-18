#![no_std]

pub use arqoii_types as types;
pub use arqoii_types::{QOI_FOOTER, QOI_MAGIC};

pub mod decode;
pub mod encode;
mod iterator_helper;
