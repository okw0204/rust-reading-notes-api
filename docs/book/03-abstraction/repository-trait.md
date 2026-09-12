# 永続化の契約を trait で読む

## 問い

service は SQL を書かずに、保存の成功・未検出・競合をどう区別できるのでしょうか。trait の各メソッドを、操作・引数・結果・失敗の4点から読みます。

## 読む場所と順序

1. `src/repository.rs` の `BookRepository` 全体。
2. `src/repository/sqlite.rs` の `impl BookRepository for SqliteBookRepository`、`insert_book`、`find_book`。
3. `src/service.rs` の `create_book` と `update_status`。

```rust,ignore
{{#include ../../../src/repository.rs:repository_contract}}
```

## 解説

### 引数と結果が責任の境界を示す

`insert_book` は `String` ではなく検証済みの `&BookTitle` と `&Author` を借り、採番済みの `StoredBook` を返します。未読で登録するという規則は rustdoc と SQLite の `INSERT` にあります。引数の型だけでは、実装が必ず未読で保存することまでは保証されません。契約の文章と実装・テストも併せて読む必要があります。

`find_book` は `Result<StoredBook, AppError>` を返し、未検出は `NotFound` です。一方、`list_notes` の空の一覧は成功です。本の存在を必ず確認する操作ではないので、service の詳細取得は先に `find_book` を呼びます。

`update_book_status` は取得時の `expected` と遷移済みの `next` を受け取ります。保存状態が一致しない場合も、取得後に削除された場合も `Conflict` です。service は遷移を判断し、repository は保存条件を確かめます。trait は SQL の文法や接続方法を指定せず、service が利用する約束を定めています。

### impl Future は結果が得られるまでの処理を表す

返り値の `impl Future<Output = Result<StoredBook, AppError>> + Send` を分解します。

| 記述 | 読み方 |
| --- | --- |
| `impl Future` | 実装側が決める、`Future` を実装した具体型を返す。呼び出し側にその型名を公開しない |
| `Output = Result<StoredBook, AppError>` | 完了時に得られる値は、成功した本かアプリケーションのエラー |
| `+ Send` | 返す Future はスレッド間で移動可能でなければならない |

`Future` は将来の結果に至る処理を表す値で、完了済みの本そのものではありません。service は `.await` で完了時の `Result` を受け取ります。SQLite 実装の `async fn insert_book(...) -> Result<...>` も、呼び出すとコンパイラが生成する Future を返すため、この契約を実装できます。`async fn` の本文は呼び出しただけでは実行されず、Future が実行器に進められると動きます。

`impl Future` は動的ディスパッチの指定ではありません。この教材は `dyn BookRepository` を使わず、`ReadingService<R>` の具体的な `R` に対してメソッドを呼びます。コンパイル時に実装が決まる静的ディスパッチです。返す Future の具体型も各実装・メソッドで決まり、呼ぶたびに任意の異なる型を選べるという意味ではありません。

### SQLite で契約を実現する

```rust,ignore
{{#include ../../../src/repository/sqlite.rs:conditional_update}}
```

`WHERE` に ID と期待する状態を入れ、`fetch_optional` の `None` を `Conflict` に変えています。SQL の失敗は `.await?` から `AppError` へ伝わり、行が返った場合は `try_into` で保存値を検証します。service はこれらの SQL の手順ではなく、成功とエラーの契約を頼りに処理を組み立てられます。

## 確認

1. `BookRepository` は SQL の書き方と、service が使う操作のどちらを定めていますか。
2. `find_book` と `list_notes` は、対象の本がない場合に同じ結果を返しますか。
3. `impl Future` を見て「実行時に実装を選ぶ」と判断できますか。
4. `update_book_status` の型だけで、期待する状態の比較が実装済みだと保証できますか。

## 解答

1. service が使う操作・引数・結果・失敗の契約です。SQLite 実装が SQL を使ってその契約を満たします。
2. いいえ。`find_book` は `NotFound`、`list_notes` は空の一覧を返す契約です。
3. できません。`impl Future` は具体型の名前を隠す返り値で、この service はジェネリクスによる静的ディスパッチを使っています。
4. できません。型は引数と結果を制約しますが、比較の意味は rustdoc・実装・競合テストで確かめます。

次は[ジェネリックな service](generic-service.md)で、具体的な実装が決まる場所を追います。
