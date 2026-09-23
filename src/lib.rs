//! 読書メモ API のライブラリ境界です。
//!
//! バイナリと統合テストの両方が、同じ Router 構築処理を利用します。

mod app;
pub mod domain;
mod error;
mod handler;
mod repository;
mod service;
#[cfg(test)]
mod test_support;

pub use app::{build_app, connect_database};
