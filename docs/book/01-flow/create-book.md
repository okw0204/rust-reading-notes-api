# 本の登録を端から端まで追う

## 問い

`POST /books` の JSON に入った文字列は、どこで検証され、どこまで所有権が移るのでしょうか。DB の行とレスポンスが、なぜ別の型なのかも考えます。

## 読む場所と順序

1. `src/app.rs` の `/books` の route。起動側は `src/main.rs`、再公開は `src/lib.rs`。
2. `src/handler.rs` の `CreateBookRequest` と `create_book`。
3. `src/service.rs` の `CreateBook` と `ReadingService::create_book`。
4. `src/repository.rs` の `insert_book` の契約、`src/repository/sqlite.rs` の実装と `BookRow`。
5. 同ファイルの `TryFrom<BookRow>`、`src/handler.rs` の `From<StoredBook> for BookResponse`。
6. 補助として `src/domain.rs` の `BookId`・`ReadingStatus`、`src/domain/text.rs` の値型。

### HTTP の入口

```rust,ignore
{{#include ../../../src/handler.rs:create_book_handler}}
```

### ユースケースの入口

```rust,ignore
{{#include ../../../src/service.rs:create_book_service}}
```

### 保存と復元

以下は `impl BookRepository for SqliteBookRepository` 内のメソッドです。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:insert_book_sqlite}}
```

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:book_row_conversion}}
```

## 解説

### JSON を読めることと、入力が規則を満たすこと

extractor はリクエストから handler の引数を取り出します。`Json<CreateBookRequest>` は Serde の `Deserialize` に従って JSON を入力 DTO に変換します。DTO は境界を越えてデータを運ぶための型です。ここでは書名と著者が `String` であることは分かっても、空白だけでないことまでは保証しません。

JSON の構文不正、必須フィールド不足、Content-Type の不一致などは Axum の rejection になり、handler 本体へ到達しません。このコードは rejection を `AppError` に統一していないので、後述の `error.code` を持つ JSON 形式になるとは限りません。一方、抽出成功後の空の書名は service の値型変換で失敗し、`AppError::Validation` として400になります。

### 所有権の移動と借用を区別する

`request.title` と `request.author` は `CreateBook` に移動し、さらに `try_from` の引数へ移動します。`String` は所有する値なので、このコードで元の DTO のフィールドをもう一度使うことはできません。変換に成功すると、正規化済み・非空の文字列を保持する `BookTitle` と `Author` が得られます。正規化では `trim` 後の文字列を作るため、「移動する」と「一切の文字列確保がない」は同じ意味ではありません。

repository には `&title` と `&author` を渡します。これは所有権を譲らずに読むための借用です。SQL の `.bind(title.as_str())` も文字列を借用し、service 側の値は `.await` を含む呼び出しの間、有効です。`.await` は非同期処理の結果を待ち、`?` は失敗ならその時点で呼び出し元へ返します。

### 同じ本を表す3つの型

| 表現 | 担当する境界と意味 |
| --- | --- |
| `BookRow` | SQLx が DB の列から復元する表現。状態も文字列で、まだ検証前 |
| `StoredBook` | 検証済みの値と、状態に応じた `Book<S>` を保持するドメイン表現 |
| `BookResponse` | HTTP で公開するフィールドを持つ `Serialize` 用 DTO |

SQL は状態を `'want_to_read'` に固定します。新規登録の入力には状態がなく、読了で直接登録する経路にはなっていません。`RETURNING` で採番された ID を含む行を取得し、`row.try_into()` が戻り値の型から `StoredBook` への変換を選びます。保存済みの値でも復元時に検証するので、不正な文字列がそのままドメインへ入りません。

handler に戻った本は `book.into()` で出力 DTO に移ります。変換では `into_parts` で分解し、値型の `into_inner` で文字列を取り出します。`BookId` は `serde(transparent)` により JSON では数値、状態は `snake_case` の文字列になります。最後の成功応答は `201 Created` です。DB 表現をそのまま JSON 化しないため、列の読み方と HTTP の公開形式を別々に決められます。

## 確認

1. `"  Rust Book  "` の前後の空白はどこで除かれますか。repository は入力の `String` を所有しますか。
2. `BookRow` をそのまま `BookResponse` として返さない理由は何ですか。
3. 不正な JSON と、空白だけの `title` は、どちらも `AppError` を通りますか。

## 解答

1. `BookTitle::try_from` から呼ばれる `normalize` で除きます。文字列は入力 DTO から値型への変換に移り、repository は完成した値型を借用します。
2. DB の行は検証前の保存表現、出力 DTO は公開表現だからです。間の `StoredBook` が値と状態の検証済みの表現を担います。
3. いいえ。不正な JSON は抽出時の rejection です。抽出できた空白だけの書名は service で検証エラーになり、`AppError` を通ります。

次の[詳細取得とエラーの出口](detail-and-errors.md)では、`?` の先にある HTTP 応答まで追います。
