use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    /// Pest grammar failed to match.
    #[error("grammar error in {product}: {detail}")]
    Grammar { product: &'static str, detail: String },

    /// A required structural field was not found.
    #[error("missing required field: {field}")]
    MissingField { field: &'static str },

    /// A value could not be converted to the expected type.
    #[error("invalid value for {field}: {value:?}")]
    InvalidValue { field: &'static str, value: String },

    /// The AWIPS ID did not match any known pattern.
    #[error("unrecognised AWIPS ID: {0}")]
    UnknownAwips(String),

    /// The storm status string was not recognised.
    #[error("unrecognised storm status: {0}")]
    UnknownStatus(String),

    /// No ZCZC line found in the product text.
    #[error("no ZCZC sentinel found in product text")]
    NoZczc,
}
