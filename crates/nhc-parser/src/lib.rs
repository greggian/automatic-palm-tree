//! NHC advisory parser — converts raw product text to typed AST nodes.
//!
//! # Usage
//!
//! ```rust,no_run
//! use nhc_parser::{parse_advisory, Advisory};
//!
//! let raw = std::fs::read_to_string("ep042025.fstadv.001.txt").unwrap();
//! let advisory = parse_advisory(&raw).unwrap();
//! println!("{:?}", advisory);
//! ```

pub use nhc_types::*;

pub mod envelope;
pub mod error;
pub mod fstadv;
pub mod public;
pub mod discus;
pub mod wndprb;

pub use error::ParseError;

/// Parse any NHC advisory product.  The product type is auto-detected from
/// the AWIPS ID in the ZCZC line.
pub fn parse_advisory(raw: &str) -> Result<Advisory, ParseError> {
    // Normalise to uppercase so grammars work with both web-archive (mixed-case
    // prose) and original operational (ALL-CAPS) advisory text.
    let raw = raw.to_uppercase();
    let product_type = envelope::detect_product_type(&raw)?;
    match product_type {
        ProductType::Tcm => fstadv::parse(&raw).map(Advisory::Fstadv),
        ProductType::Tcp => public::parse(&raw).map(Advisory::Public),
        ProductType::Tcd => discus::parse(&raw).map(Advisory::Discus),
        ProductType::Pws => wndprb::parse(&raw).map(Advisory::Wndprb),
    }
}
