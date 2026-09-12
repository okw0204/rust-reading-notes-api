# ジェネリックな service を具体化する

## 問い

`ReadingService<R>` の `R` はどこで決まるのでしょうか。フェイクと SQLite で共通の判断を実行する仕組みと、所有権の行き先を追います。

## 読む場所と順序

1. `src/service.rs` の `ReadingService<R>`、`new`、`create_book`。
2. `src/app.rs` の `AppState` と `build_app`。
3. `src/handler.rs` の `State<AppState>` を受け取る handler。
4. `src/service/tests.rs` の `reports_a_save_conflict_without_changing_the_book`。

```rust,ignore
{{#include ../../../src/service.rs:generic_service}}
```

この抜粋は `impl` の先頭までです。続くユースケースのメソッドも同じ `impl<R: BookRepository>` の中にあります。

## 解説

### 型引数と所有する値を区別する

`R` は repository の型を表す型引数、`repository: R` は service が所有する値です。`new(repository: R)` は参照ではなく値を受け取り、フィールドへ移動します。struct の宣言自体には境界がなく、`new` とユースケースを定義する `impl` に `R: BookRepository` が付いています。そのため、これらのメソッドを利用できるのは契約を実装する `R` の場合です。

service の各メソッドは `&self` を受け取り、所有する repository を共有借用して呼びます。呼び出すたびに repository を移動したり複製したりする必要はありません。trait の境界にも service にも `Clone` は要求されていません。

### 起動時に SQLite へ具体化する

```rust,ignore
{{#include ../../../src/app.rs:composition}}
```

`SqliteBookRepository::new(pool)` の結果を `ReadingService::new` に渡すと、`R` は `SqliteBookRepository` に決まります。これは実行時の型切り替えではなく、式からコンパイラが型を推論することです。`AppState` のフィールドにも `Arc<ReadingService<SqliteBookRepository>>` と具体型が書かれています。

handler は `State<AppState>` を受け取るので、HTTP 経路は SQLite に具体化されています。repository trait があるからといって、すべての層がジェネリックなわけではありません。

`AppState` の `Clone` は中の `Arc` を clone します。同じ service への共有所有権が増えるのであって、service や DB 全体のコピーを作るのではありません。`SqlitePool` 自身も接続を共有するハンドルです。service の共有と接続の共有は別の段階にあります。

### フェイクにも同じ service を使う

テストの `ReadingService::new(fake.clone())` では `R` が `FakeBookRepository` になります。`ReadingService<SqliteBookRepository>` と `ReadingService<FakeBookRepository>` は異なる具体型ですが、入力の検証や状態遷移の判断には同じジェネリックな実装を使います。

ここで clone するのはフェイクです。テスト側に失敗注入・保存結果の検査用ハンドルを残し、もう一つを service へ移動しています。フェイク内部の `Arc` が同じメモリ状態を共有するため、テスト側の設定が service 側の呼び出しに反映されます。

## 確認

1. `ReadingService<R>` は repository を所有しますか、それとも参照だけを持ちますか。
2. `ReadingService` に `Clone` がなくても `AppState` を clone できるのはなぜですか。
3. フェイクを使うテストは、service の判断も別実装に差し替えていますか。
4. HTTP の handler が使う `R` は何ですか。どの型宣言が根拠になりますか。

## 解答

1. `repository: R` として所有します。メソッド実行時は `&self` から借用します。
2. `AppState` が持つのは `Arc` で、clone に service 自身の `Clone` を必要としないためです。
3. いいえ。repository の実装だけを変え、同じ `ReadingService<R>` の判断を検査しています。
4. `SqliteBookRepository` です。`AppState.service` の `Arc<ReadingService<SqliteBookRepository>>` と handler の `State<AppState>` が根拠です。

次は[非同期処理の借用と共有](async-bounds.md)で、`Send`・`Sync`・`'static` の要求を分けて読みます。
