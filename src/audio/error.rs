use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum AudioError {
    #[error("Audio output device error: {0}")]
    DeviceError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Decoding error: {0}")]
    DecodingError(String),

    #[error("Track not found: {0}")]
    TrackNotFound(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
