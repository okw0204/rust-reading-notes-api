# Rust Reading Notes API

Rust Book を一通り読んだあとに、完成した Web API を HTTP の入口から DB、テストまで追うための日本語コードリーディング教材です。読書記録を題材に、Axum・SQLx・SQLite と Rust の型を結びつけます。

```text
Router → handler → ReadingService<R> → BookRepository の SQLite 実装 → DB
            ↑         結果とエラーが呼び出し元へ戻り、HTTP 応答になる
```

認証や外部 API は扱わず、値の移動、型の保証、保存の契約を読むことに集中します。第1部は60〜90分、全4部12章は3〜5時間程度が目安です。復習や実験の量に合わせて調整してください。

## コードリーディング教材

[教材の使い方](docs/book/introduction.md)と[目次](docs/book/SUMMARY.md)から読み始められます。
ローカルで目次・検索・図付きの教材を開くには、mise でツールを導入します。

**初回のみ：設定の信頼とツールのインストール**

リポジトリのルートで実行します。

```bash
mise trust mise.toml
mise install
```

**教材を起動する：2回目以降はこれだけ**

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
| 第1部 | [依存関係の組み立て](docs/book/01-flow/composition.md)、登録時の所有権移動、詳細取得とエラー変換 |
| 第2部 | [検証済みの値型](docs/book/02-types/validated-values.md)、`Book<S>` の型状態、DB の実行時状態との境界 |
| 第3部 | [repository trait](docs/book/03-abstraction/repository-trait.md)、ジェネリックな service、Future の借用と `Send`・`Sync`・`Arc` |
| 第4部 | [service のフェイク](docs/book/04-tests/service-fake.md)、SQLite の条件付き更新、Router の HTTP 統合テスト |

各章は「問い → 読む場所と順序 → 解説 → 確認 → 解答」の順です。完成した実装とテストを根拠に読み進められます。

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

curl -i -X POST http://127.0.0.1:3000/books/1/notes \
  -H 'content-type: application/json' \
  -d '{"body":"所有権の説明を再読する"}'

curl -i -X PATCH http://127.0.0.1:3000/books/1/status \
  -H 'content-type: application/json' \
  -d '{"status":"finished"}'

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
| `DELETE` | `/books/{id}` | `204` | 本と関連メモを削除する（本文なし） |

登録時の状態は `want_to_read` 固定です。状態更新は `WantToRead → Reading → Finished` の順にだけ許可します。

| 現在の状態 ＼ 要求する状態 | `want_to_read` | `reading` | `finished` |
| --- | --- | --- | --- |
| `want_to_read`（未読） | `409` | `200` | `409` |
| `reading`（読書中） | `409` | `409` | `200` |
| `finished`（読了） | `409` | `409` | `409` |

認識できない状態名や空白だけの入力は `400 Bad Request` です。対象取得時の未検出は `404 Not Found`、取得後の条件付き更新が成立しない場合は途中の削除も含め `409 Conflict` です。保存値の不正や DB エラーは内部詳細を隠して `500 Internal Server Error` に変換します。

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
│   ├── tests.rs        # service の判断と失敗の検査
│   └── tests/fake.rs   # メモリ上の repository
├── repository.rs       # BookRepository の契約
├── repository/sqlite.rs # SQL、行変換、DB テスト
└── error.rs            # エラーと HTTP 応答の変換

migrations/             # SQLite のスキーマと制約
tests/api.rs            # 実 DB と Router の統合テスト
docs/book/              # 全4部12章と導入・目次
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

`cargo test` は型状態の doctest（合法な操作と `compile_fail`）、service のフェイク、SQLite の保存契約、Router の統合テストを実行します。API テストは独立したインメモリ SQLite（`sqlite::memory:`）を1接続で使い、repository の `#[sqlx::test]` は SQLx が独立したファイル DB と pool を用意します。後者は `connect_database` の1接続設定を引き継ぎません。どちらもアプリと同じ migration を適用し、手元の `reading-notes.db` を共有しません。フェイクでは保存失敗を注入し、repository テストでは実 SQL、API テストでは HTTP と保存結果を観測します。

`mdbook build` の生成先は `book/` で、Git 管理対象外です。検証の読み方は[第4部](docs/book/04-tests/service-fake.md)で確認できます。
