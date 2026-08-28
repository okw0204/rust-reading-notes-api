# Rust Reading Notes API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Axum から SQLite までの処理と Rust らしい型設計を読める、小型の読書メモ API を作る。

**Architecture:** HTTP、ユースケース、DB 操作、ドメイン型を明示的なレイヤーへ分ける。完成コードと日本語の読解ガイドを一緒に提供する。

**Tech Stack:** Rust 2024、Axum、Tokio、SQLx、SQLite、Serde、Thiserror、Tower

**Spec:** `docs/superpowers/specs/2026-08-28-rust-reading-notes-api-design.md`

## Global Constraints

- 1 周 60〜90 分で主要なコードを読める規模に保つ。
- 認証、外部 API、Docker は追加しない。
- コードコメントと人向け文書は日本語で書く。
- production code は失敗するテストを確認してから実装する。

---

### Task 1: DB とアプリケーション骨格

**Files:** `migrations/0001_create_books_and_notes.sql`、`src/lib.rs`、`src/app.rs`、`tests/common/mod.rs`、`tests/api.rs`

- [ ] 空の Router に対して期待する route が存在しないことを確認するテストを書く
- [ ] テストが期待した理由で失敗することを確認する
- [ ] migration、共有状態、Router、テスト用 DB helper を最小実装する
- [ ] 対象テストと全テストを通す

### Task 2: ドメイン型とエラー

**Files:** `src/domain.rs`、`src/error.rs`

- [ ] `ReadingStatus`の変換と`AppError`の HTTP 変換テストを書く
- [ ] テストが期待した理由で失敗することを確認する
- [ ] newtype、enum、ドメイン struct、エラー response を実装する
- [ ] 対象テストと全テストを通す

### Task 3: 本の登録と一覧

**Files:** `src/handler.rs`、`src/service.rs`、`src/repository.rs`、`tests/api.rs`

- [ ] 登録、入力不正、一覧、状態絞り込みの API テストを書く
- [ ] テストが route または実装不足で失敗することを確認する
- [ ] handler、service、repository を通る最小実装を追加する
- [ ] 対象テストと全テストを通す

### Task 4: 詳細取得とメモ追加

**Files:** `src/handler.rs`、`src/service.rs`、`src/repository.rs`、`tests/api.rs`

- [ ] 詳細、未検出、メモ追加、入力不正の API テストを書く
- [ ] テストが期待した理由で失敗することを確認する
- [ ] 本とメモの取得および追加を実装する
- [ ] 対象テストと全テストを通す

### Task 5: 状態更新と削除

**Files:** `src/handler.rs`、`src/service.rs`、`src/repository.rs`、`tests/api.rs`

- [ ] 状態更新、削除、cascade、未検出の API テストを書く
- [ ] テストが期待した理由で失敗することを確認する
- [ ] 更新と削除を実装する
- [ ] 対象テストと全テストを通す

### Task 6: 読解ガイドと最終検証

**Files:** `README.md`、`src/*.rs`

- [ ] README に起動方法、API、読む順番、確認問題を記載する
- [ ] 設計判断が分かりにくい箇所へ日本語コメントを追加する
- [ ] `cargo fmt -- --check`、`cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`を通す
- [ ] サーバー起動と主要 API を手動確認する
