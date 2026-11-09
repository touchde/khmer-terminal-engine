//! C ABI for embedding Khmer Terminal Engine in native terminals.
//!
//! This is the only crate allowed to use `unsafe`; every `unsafe` block must
//! carry a `// SAFETY:` comment.
