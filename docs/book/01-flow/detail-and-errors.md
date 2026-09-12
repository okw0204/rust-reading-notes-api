# 詳細取得とエラーの出口

## 問い

本がない場合と、保存された本の値が壊れている場合は、どこで区別されるのでしょうか。本とメモが両方返れば、同じ時点のデータだと言えるかも考えます。

## 読む場所と順序

1. 起動経路の `src/main.rs`・`src/lib.rs` から接続される `src/app.rs` の `/books/{id}`。
2. `src/handler.rs` の `get_book` と `BookDetailResponse`、`src/domain.rs` の `BookId`・`BookDetail`。
3. `src/service.rs` の `get_book`。
4. `src/repository/sqlite.rs` の `find_book`、`list_notes`、`TryFrom<BookRow>`・`TryFrom<NoteRow>`。
5. `src/error.rs` の `AppError`、`From<InvalidText>`、`IntoResponse`。

```rust,ignore
{{#include ../../../src/service.rs:get_book_service}}
```

以下は SQLite repository のメソッドです。

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:find_book_sqlite}}
```

```rust,ignore
{{#include ../../../src/error.rs:http_error_mapping}}
```

## 解説

### Option と Result は答えている問いが違う

`fetch_optional` の結果は `Result<Option<BookRow>, sqlx::Error>` です。外側の `Result` は「SQL の実行は成功したか」、内側の `Option` は「該当する行があったか」を表します。正常に検索できて行が0件なら `Ok(None)` であり、SQL の失敗ではありません。

最初の `.await?` は DB エラーを伝播し、成功した `Option` を取り出します。続く `.ok_or(AppError::NotFound)?` が、`Some(row)` なら行を取り出し、`None` なら未検出を返します。未検出の分類を行うのは repository です。service は既に `Result<StoredBook, AppError>` になった結果を受け取ります。

`?` はログ出力や HTTP 応答生成そのものではありません。`Err` なら必要に応じて `From` でエラー型を変換して早期に返す記法です。SQLx のエラーは `Database(#[from] sqlx::Error)` により `AppError` に変換されます。既に `AppError` であれば同じ型のまま、repository → service → handler と伝播します。

### 詳細は2回の問い合わせで作る

`get_book` は本の取得に成功してからメモ一覧を取得します。本がなければ最初の `?` で戻るので、メモの SQL は実行されません。メモが0件なら空の `Vec` が正常な結果です。各メモの本文も `NoteBody` に変換され、不正な保存値なら詳細全体が失敗します。

この処理は別々の SELECT を使い、2つを囲むトランザクションを開始していません。最大1接続の pool でも、各問い合わせの間に別の処理が入る可能性を排除しません。本の取得後に削除やメモ追加があれば、返す本とメモが一つのスナップショットに由来する保証はありません。`.await` の記述順はこの呼び出し内の順序であり、他のリクエスト全体を止める指定ではないのです。

### エラーの原因と HTTP の出口

| 原因 | 分類 | 応答 |
| --- | --- | --- |
| 対象の本が見つからない | `NotFound` | 404 / `not_found` |
| 入力文字列が値型の規則に違反、未知の入力状態名 | `Validation` | 400 / `validation_error` |
| 許可されない遷移、条件付き更新の不成立 | `Conflict` | 409 / `conflict` |
| DB の書名・著者・メモ本文・状態が不正 | `InvalidStoredValue` | 500 / `internal_error` |
| SQL 実行などの DB 障害 | `Database` | 500 / `internal_error` |

`IntoResponse` は型を HTTP 応答へ変える Axum の trait です。handler の `Result` が `Err(AppError)` のとき、この実装が使われます。500の詳細はサーバー側に記録し、外には `internal server error` を返します。入力の空文字列と DB 内の空文字列は同じ値型の検証に失敗しますが、後者を利用者の入力ミスである400にはしません。

`Path<BookId>` の抽出失敗（数値として読めない ID）や `Json` の失敗は handler 本体の前で Axum が rejection を返します。この表は `AppError` の表であり、全 HTTP エラーが同じ JSON 形式になる保証ではありません。

## 確認

1. `Ok(None)` はどの行で何のエラーになりますか。メモの取得は続きますか。
2. 入力の空書名と DB の空書名で、なぜ400と500を分けますか。
3. 本とメモの取得に成功したら、同じ時点のデータと断言できますか。
4. `?` を通るたびに HTTP 応答が作られますか。

## 解答

1. repository の `ok_or(AppError::NotFound)?` で未検出になります。service の最初の `?` で戻るため、メモ取得は続きません。
2. 前者はリクエストの検証失敗、後者は保存データの不正だからです。値型の規則は共通でも、原因の分類は呼び出し境界が決めます。
3. 断言できません。2つの SELECT をまとめたスナップショットの保証がありません。
4. いいえ。`?` はエラーを返し、HTTP 応答への変換は Axum の応答境界で `IntoResponse` が行います。

第1部の仕上げに、登録と詳細取得の経路をソースを閉じて説明してみてください。次は[検証済みの値型](../02-types/validated-values.md)で、成功した値に何を期待できるかを掘り下げます。
