pub mod backend_mock;
pub mod impl_;

pub use impl_::adopt;

#[cfg(test)]
mod tests;
