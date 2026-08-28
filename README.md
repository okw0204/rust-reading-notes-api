# Rust Reading Notes API

Rust Book を一通り読んだあとに、Web API のコードを入口からデータベースまで追うためのコードリーディング教材です。

読書記録を題材に、次の処理経路を明示的なレイヤーへ分けています。

```text
HTTP request
  ↓
Router
  ↓
handler        Path・Query・JSONをRustの型へ変換
  ↓
service        入力検証とユースケース
  ↓
repository     SQLxによるSQLite操作
  ↓
HTTP response
```

本格的な読書管理サービスではありません。認証や外部 API を省き、60〜90 分で主要な処理を一周できる規模を優先しています。

## 学べること

- Axum の`Router`、extractor、handlerがどうつながるか
- async関数を通ってSQLxのDB操作へ到達する流れ
- `BookId`と`NoteId`をnewtypeにする理由
- 文字列で保存される状態を`ReadingStatus`で制約する方法
- HTTP用、DB用、ドメイン用の型を分ける境界設計
- `AppError`をHTTPステータスとJSONへ変換する方法
- Routerを直接呼ぶ統合テストの組み立て方

## 起動

必要なものはRust toolchainだけです。SQLiteはSQLxが組み込みで利用します。

```bash
cargo run
```

サーバーは`http://127.0.0.1:3000`で起動し、リポジトリ直下に`reading-notes.db`を作成します。

本を登録します。

```bash
curl -i http://127.0.0.1:3000/books \
  -H 'content-type: application/json' \
  -d '{"title":"Rust for Rustaceans","author":"Jon Gjengset"}'
```

登録した本を一覧表示します。

```bash
curl http://127.0.0.1:3000/books
curl 'http://127.0.0.1:3000/books?status=want_to_read'
```

状態更新、メモ追加、詳細取得、削除も試せます。

```bash
curl -X PATCH http://127.0.0.1:3000/books/1/status \
  -H 'content-type: application/json' \
  -d '{"status":"reading"}'

curl -X POST http://127.0.0.1:3000/books/1/notes \
  -H 'content-type: application/json' \
  -d '{"body":"所有権の説明を再読する"}'

curl http://127.0.0.1:3000/books/1
curl -i -X DELETE http://127.0.0.1:3000/books/1
```

読書状態には`want_to_read`、`reading`、`finished`のいずれかを指定します。

## API

| Method | Path | 成功時 | 処理 |
| --- | --- | --- | --- |
| `POST` | `/books` | `201` | 本を登録する |
| `GET` | `/books` | `200` | 本を一覧表示する |
| `GET` | `/books?status=reading` | `200` | 状態で絞り込む |
| `GET` | `/books/{id}` | `200` | 本とメモを取得する |
| `PATCH` | `/books/{id}/status` | `200` | 読書状態を変更する |
| `POST` | `/books/{id}/notes` | `201` | メモを追加する |
| `DELETE` | `/books/{id}` | `204` | 本と関連メモを削除する |

アプリケーションが返すエラーは次の形に統一しています。

```json
{
  "error": {
    "code": "validation_error",
    "message": "title must not be empty"
  }
}
```

## ファイル構成

```text
src/
├── main.rs        # DB接続とサーバー起動
├── lib.rs         # バイナリと統合テストの共有境界
├── app.rs         # RouterとAppState
├── domain.rs      # ID、読書状態、本、メモ
├── handler.rs     # HTTPの入力・出力
├── service.rs     # 入力検証とユースケース
├── repository.rs  # SQLとDB行の変換
└── error.rs       # エラーとHTTP responseの変換

migrations/        # SQLite schema
tests/api.rs       # 実DBとRouterを使う統合テスト
```

## 60〜90分の読解順路

### 1. 起動と依存関係の組み立て（10分）

`src/main.rs`、`src/lib.rs`、`src/app.rs`の順に読みます。

読む前の問い:

- バイナリとは別に`lib.rs`があると、テストにどんな利点があるでしょうか。
- `SqlitePool`はどこで作られ、どこまで移動するでしょうか。

確認:

- `build_app`をテストと本番が共有する理由を説明できますか。
- `AppState`が`Clone`を必要とする理由を推測できますか。

### 2. 本の登録を端から端まで追う（20分）

`app.rs`の`POST /books`から始め、`handler::create_book`、`service::create_book`、`repository::insert_book`へ進みます。

読む前の問い:

- 空文字の検証はhandler、service、repositoryのどこへ置くべきでしょうか。
- `CreateBookRequest`をそのままrepositoryへ渡さない理由は何でしょうか。

確認:

- `String`はリクエストからどこへ所有権を移していますか。
- SQLxの`BookRow`はどこで`Book`へ変わりますか。

### 3. 型の境界を読む（15分）

`src/domain.rs`と、`handler.rs`、`repository.rs`にある`From`・`TryFrom`実装を読みます。

読む前の問い:

- `BookId(i64)`は`i64`だけを使う場合と何が違うでしょうか。
- DBの`status`が任意の文字列であることを、どの境界で止めるべきでしょうか。

確認:

- 利用者の不正入力と、DBに保存された不正値はなぜ別のエラーですか。
- `#[serde(transparent)]`と`#[serde(rename_all = "snake_case")]`はJSONをどう変えますか。

### 4. 詳細取得とエラーを追う（15分）

`GET /books/{id}`を追い、最後に`src/error.rs`を読みます。

読む前の問い:

- 本が存在しないことはSQLxではどの型で表せるでしょうか。
- 内部のDBエラーをそのまま利用者へ返すと何が問題でしょうか。

確認:

- `Option`はどこで`AppError::NotFound`へ変わりますか。
- `?`が各レイヤーを早期returnする流れを説明できますか。

### 5. テストから逆向きに読む（15〜30分）

`tests/api.rs`を開き、`adds_a_note_to_a_book`と`deletes_a_book_and_its_notes`を読みます。

読む前の問い:

- 実際のTCPポートを開かずに、どうやってHTTP処理をテストしているでしょうか。
- インメモリSQLiteを1接続に固定する理由は何でしょうか。

確認:

- テストはhandlerだけでなく、どの範囲を実際に通っていますか。
- 本を削除した際のメモ削除を、なぜレスポンスだけでなくDBでも確認していますか。

## テスト

```bash
cargo fmt -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

統合テストはテストごとに独立したインメモリSQLiteを作り、同じmigrationを適用します。外部サービスやmockは使いません。

## 発展課題

1. `GET /books`へページネーションを追加する。
2. タイトルまたは著者の部分一致検索を追加する。
3. repositoryをtraitにして、serviceの単体テスト用実装とSQLite実装を差し替える。
4. `created_at`をAPIレスポンスへ追加し、日時表現の境界を設計する。
5. 本とメモの詳細取得に一貫性が必要な場合、トランザクションをどう使うか検討する。

抽象化を増やす前に、現在の具体的な実装でどの依存を切り離したいのか説明できる状態を目指してください。
