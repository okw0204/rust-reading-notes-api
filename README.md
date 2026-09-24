# Rust Reading Notes API

Rust Book を一通り読んだあとに、一括読了記録という一つの完成した処理を、小さな入力値から HTTP、SQLite、複数の Future まで段階的に追う日本語コードリーディング教材です。読書記録を題材に、所有権、型、抽象化、非同期処理を Axum・SQLx・SQLite の実装と結びつけます。

```text
Router → handler → ReadingService<R> → BookRepository の SQLite 実装 → DB
            ↑         結果とエラーが呼び出し元へ戻り、HTTP 応答になる
```

認証や外部 API は扱わず、値の移動、型の保証、保存の契約を読むことに集中します。読むだけで完結し、複数日に分けて進められる構成です。未測定の読了時間は固定の目安として示していません。

## コードリーディング教材

[完成形と一括読了記録](docs/book/introduction.md)と[目次](docs/book/SUMMARY.md)から読み始められます。
ローカルで目次・検索・図付きの教材を開くには、mise でツールを導入します。

### 初回のみ：設定の信頼とツールのインストール

リポジトリのルートで実行します。

```bash
mise trust mise.toml
mise install
```

### 教材を起動する：2 回目以降はこれだけ

```bash
mise exec -- mdbook serve --hostname 127.0.0.1 --port 3001
```

`mdbook serve` はビルドと変更時の自動再ビルドも行います。教材の閲覧には API の起動は不要です。
mise の環境がシェルで有効なら、`mdbook serve --port 3001` でも起動できます。
`http://127.0.0.1:3001/introduction.html` を開き、終了時は `Ctrl+C` を押します。
Markdown 原文ではソースの `include` や Mermaid の図が生成 HTML と同じ表示にはならないため、抜粋と図はローカルの教材で確認してください。
検索で日本語の語句が見つからない場合は、`Future` や `BookRepository` などのコード識別子、または目次を使ってください。

## 学べること

| 部 | 内容と入口 |
| --- | --- |
| 第 1 部：一件の値を整える | [要求から検証済みの入力へ](docs/book/01-values/reading-completion-input.md)、[借用して検査し、所有して渡す](docs/book/01-values/borrow-and-own.md) |
| 第 2 部：一冊の読了を成立させる | [保存状態を型状態へ接続する](docs/book/02-types/stored-state-to-typestate.md)、[原子的な保存](docs/book/02-types/atomic-completion.md)、[失敗後の保存結果](docs/book/02-types/completion-failures.md) |
| 第 3 部：Interface の向こうを読む | [`BookRepository` の契約](docs/book/03-abstraction/repository-trait.md)、[フェイクと SQLite Adapter](docs/book/03-abstraction/adapters.md) |
| 第 4 部：複数の処理を進める | [Future が値を保持する範囲](docs/book/04-async/future-values.md)、[独立した処理の並行進行](docs/book/04-async/concurrent-completions.md)、[結果順](docs/book/04-async/completion-order.md)、[失敗と親 Future の終了](docs/book/04-async/failure-and-parent-future.md) |
| 第 5 部：利用者の経路を読み直す | [HTTP から SQLite まで](docs/book/05-flow/http-to-sqlite.md)、[検証の保証と限界](docs/book/05-flow/verification-limits.md) |

各章は「問い → 読む場所と順序 → 解説 → 確認 → 解答」の順です。完成した実装と検証済みの例を根拠に読み進められます。

## 起動

Rust の現在の stable を推奨します（Edition 2024 と依存クレートの要求を満たすもの）。SQLite は SQLx が組み込みで利用し、別の DB サーバーは不要です。コマンドはリポジトリのルートで実行します。

```bash
cargo run
```

API は `http://127.0.0.1:3000` で起動し、`reading-notes.db` を作成して migration を適用します。起動時の `connect_database` と API テストの準備関数 `test_app_and_pool` は、それぞれ pool を最大1接続に設定します。repository の `#[sqlx::test]` は SQLx が別の設定で pool を準備します。接続数の制限は、複数の SQL をまたぐユースケース全体の原子性を保証するものではありません。

別の端末で、未読の本を登録します。

```bash
curl -i http://127.0.0.1:3000/books \
  -H 'content-type: application/json' \
  -d '{"title":"Rust for Rustaceans","author":"Jon Gjengset"}'

curl http://127.0.0.1:3000/books
curl 'http://127.0.0.1:3000/books?status=want_to_read'
```

以降の `1` は登録応答の `id` に置き換えてください。未読から読書中、読了の順に進めます。

```bash
curl -i -X PATCH http://127.0.0.1:3000/books/1/status \
  -H 'content-type: application/json' \
  -d '{"status":"reading"}'

curl -i -X POST http://127.0.0.1:3000/reading-completions \
  -H 'content-type: application/json' \
  -d '{"items":[{"book_id":1,"body":"所有権と Future の関係を確認した"}]}'

curl http://127.0.0.1:3000/books/1
curl -i -X DELETE http://127.0.0.1:3000/books/1
```

## API

| メソッド | パス | 成功時 | 処理 |
| --- | --- | --- | --- |
| `POST` | `/books` | `201` | 本を未読で登録する |
| `GET` | `/books` | `200` | 本を一覧表示する |
| `GET` | `/books?status=reading` | `200` | 状態で絞り込む |
| `GET` | `/books/{id}` | `200` | 本とメモを取得する |
| `PATCH` | `/books/{id}/status` | `200` | 読書状態を変更する |
| `POST` | `/books/{id}/notes` | `201` | メモを追加する |
| `POST` | `/reading-completions` | `200` | 1〜8 冊を読了時のメモとともに記録する |
| `DELETE` | `/books/{id}` | `204` | 本と関連メモを削除する（本文なし） |

登録時の状態は `want_to_read` 固定です。状態更新は `WantToRead → Reading → Finished` の順にだけ許可します。

| 現在の状態 ＼ 要求する状態 | `want_to_read` | `reading` | `finished` |
| --- | --- | --- | --- |
| `want_to_read`（未読） | `409` | `200` | `409` |
| `reading`（読書中） | `409` | `409` | `200` |
| `finished`（読了） | `409` | `409` | `409` |

一括読了記録は、重複しない 1 件以上 8 件以下の `book_id` と、空でない `body` を受け取ります。要求全体を保存前に検証し、不正なら `400 Bad Request` としてどの本も変更しません。入力全体が正しければ、最大 8 件の独立した処理を親 Future の中で並行に進めます。完了順にかかわらず、各項目の `outcome` を入力順で `results` に返し、一部または全部が失敗しても要求は `200 OK` です。

一冊の未検出は `not_found`、読書状態の不整合は `conflict`、保存値の不正や DB エラーは内部詳細を隠した `internal_error` として、その項目の `error` に入ります。一冊の読書状態とメモは同じ transaction で保存し、失敗時はどちらも残しません。別の本ですでに確定した読了記録は取り消しません。

一括読了記録以外では、対象取得時の未検出は `404 Not Found`、条件付き更新が成立しない場合は `409 Conflict`、保存値の不正や DB エラーは `500 Internal Server Error` です。

アプリケーションのエラーは次の JSON 形式です。

```json
{
  "error": {
    "code": "validation_error",
    "message": "title must not be empty"
  }
}
```

不正な JSON、`Content-Type` の不足、数値でない Path など、handler 本文より前の失敗には Axum 標準の rejection が使われます。この場合、上記の JSON 形式は保証しません。

## ファイル構成

```text
src/
├── main.rs             # DB 接続とサーバー起動
├── lib.rs              # バイナリと統合テストの共有境界
├── app.rs              # Router と共有状態の組み立て
├── domain.rs           # ID、読書状態、ドメイン型の入口
├── domain/
│   ├── book.rs         # Book<S> と StoredBook
│   └── text.rs         # 検証済みの文字列
├── handler.rs          # HTTP の入力・出力
├── service.rs          # ReadingService<R> とユースケース
├── service/
│   └── tests.rs        # service の判断、並行進行、終了の検査
├── repository.rs       # BookRepository の契約
├── repository/sqlite.rs # SQL、行変換、DB テスト
└── error.rs            # エラーと HTTP 応答の変換

migrations/             # SQLite のスキーマと制約
tests/api.rs            # 実 DB と Router の統合テスト
docs/book/              # 全 5 部の教材と導入・目次
book.toml               # mdBook の設定
mise.toml               # 教材ビルド用ツールの固定
```

## 検証方法

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
mise exec -- mdbook build
git diff --check
```

`cargo test` は型状態の doctest（合法な操作と `compile_fail`）、service のフェイク、SQLite の保存契約、Router の統合テストを実行します。API テストは独立したインメモリ SQLite（`sqlite::memory:`）を 1 接続で使い、repository の `#[sqlx::test]` は SQLx が独立したファイル DB と pool を用意します。後者は `connect_database` の 1 接続設定を引き継ぎません。どちらもアプリと同じ migration を適用し、手元の `reading-notes.db` を共有しません。フェイクでは失敗と Future の進行を制御し、repository テストでは実 SQL、API テストでは HTTP と保存結果を観測します。

`mdbook build` の生成先は `book/` で、Git 管理対象外です。検証の読み方は[検証の保証と限界](docs/book/05-flow/verification-limits.md)で確認できます。
