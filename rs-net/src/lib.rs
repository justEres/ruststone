#![recursion_limit = "256"]

mod auth;
mod chunk_decode;
mod handle_packet;
mod outbound;
mod session;

pub use session::start_networking;
