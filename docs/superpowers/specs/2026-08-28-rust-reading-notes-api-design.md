# Rust Reading Notes API 設計書

## 目的

Rust Book を一通り読んだ学習者が、Web リクエストの入口から SQLite まで処理を追うためのコードリーディング教材を作る。1 周 60〜90 分を想定し、`Router → handler → service → repository → SQLite`の流れを主題、newtype や enum などの型設計を副題とする。

## スコープ

1 人用のローカル JSON API とし、認証、外部 API、Docker は扱わない。題材は本と読書メモで、次の操作を提供する。

| Method | Path | 処理 |
| --- | --- | --- |
| `POST` | `/books` | 本を登録する |
| `GET` | `/books?status=reading` | 本を一覧・絞り込み表示する |
| `GET` | `/books/{id}` | 本とメモを取得する |
| `PATCH` | `/books/{id}/status` | 読書状態を変更する |
| `POST` | `/books/{id}/notes` | メモを追加する |
| `DELETE` | `/books/{id}` | 本とメモを削除する |

## アーキテクチャ

- `main.rs`: DB 接続、migration、ログ、サーバー起動
- `app.rs`: Router と共有状態の組み立て
- `handler.rs`: Path、Query、JSON と HTTP response の変換
- `service.rs`: 入力検証とユースケース
- `repository.rs`: SQLx による SQLite 操作
- `domain.rs`: HTTP や DB に依存しないドメイン型
- `error.rs`: アプリケーションエラーから HTTP response への変換
- `lib.rs`: 統合テストから利用する公開境界

## データモデル

`BookId`と`NoteId`は整数を包む newtype とする。読書状態は`ReadingStatus::{WantToRead, Reading, Finished}`で表し、SQLite には`want_to_read`、`reading`、`finished`という文字列で保存する。

`books`は`id`、`title`、`author`、`status`、`created_at`を持つ。`notes`は`id`、`book_id`、`body`、`created_at`を持ち、`book_id`には`ON DELETE CASCADE`付きの外部キーを設定する。

## エラー

`AppError`へ入力不正、未検出、DB エラー、DB 内の不正値を集約する。それぞれ`400 Bad Request`、`404 Not Found`、`500 Internal Server Error`へ変換し、`{"error":{"code":"...","message":"..."}}`形式で返す。内部 DB エラーの詳細はクライアントへ公開しない。

## コードリーディング支援

README に読む順番、読む前の問い、読んだ後の確認問題を載せる。コードコメントは日本語とし、モジュールの責務、所有権、型変換、extractor、エラー境界など、コードだけでは理由が分かりにくい箇所に限定する。構文を言い換えるだけのコメントは避ける。

## テスト

ドメイン変換と service の入力検証は単体テストする。repository は独立したインメモリ SQLite で検証する。API 統合テストは Router に直接 request を送り、HTTP status と JSON を検証する。

最終確認は次のコマンドで行う。

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```
