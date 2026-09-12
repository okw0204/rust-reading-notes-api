# 非同期処理の借用と共有を読む

## 問い

service 内のローカル値を repository に貸したまま `.await` できるのはなぜでしょうか。Future の制約と、Router に渡す共有状態の制約を区別します。

## 読む場所と順序

1. `src/service.rs` の `create_book`。
2. `src/repository.rs` の `BookRepository: Send + Sync` と `insert_book` の返り値。
3. `src/repository/sqlite.rs` の `insert_book`。
4. `src/app.rs` の `AppState` と `build_app`、`src/handler.rs` の `create_book`。

```rust,ignore
{{#include ../../../src/service.rs:create_book_service}}
```

## 解説

### await 中も借りた値が有効であること

`async fn create_book` を呼ぶと、引数や処理の途中経過を保持する Future が返ります。Future が進められると `title` と `author` を作り、repository の Future を `.await` します。未完了なら呼び出し元の処理も一時停止し、再開に必要な値や借用を保ちます。`.await` は必ず停止する命令ではなく、相手が既に完了できればそのまま先へ進みます。

`insert_book(&title, &author)` の Future は、service が所有する repository とローカル値を借用できます。service はその場で完了を待つため、借用先は待機中も有効です。SQLite 実装も `title.as_str()` などを SQL に bind して `.await` しています。

trait の返り値には `+ Send` があり、無条件の `+ 'static` はありません。この trait 内の `impl Future` は引数の参照のライフタイムを取り込めます。独立したタスクに切り離すのではなく、呼び出し元の Future の内側で借用した Future を待つ形です。参照先が無効になる形で Future を持ち出すことは、借用検査が拒否します。

### Send と Sync は別の対象を見る

| 制約 | 意味 | この教材で見る場所 |
| --- | --- | --- |
| `T: Send` | `T` の所有権をスレッド間で移動できる | repository 自身と各操作が返す Future |
| `T: Sync` | `&T` をスレッド間で渡して共有できる | 共有借用される repository |
| `T: 'static` | `T` が非 static な参照に依存しない | Router に保持させる共有状態 |

repository の Future が待機中に `&self` を保持するとき、その参照が `Send` であるには参照先の repository が `Sync` である必要があります。一般に `&T: Send` の条件は `T: Sync` です。`BookRepository: Send + Sync` と各メソッドの `+ Send` は、このようにつながります。ただし repository が `Sync` でさえあれば、どんなメソッドの Future も `Send` になるわけではありません。Future が停止をまたいで保持するほかの値や参照も条件を満たす必要があります。

`Send` は移動を許す条件であり、毎回スレッドが変わる保証ではありません。`.await` はスレッドを作る操作でもありません。実行器が処理を進め、必要なら待機し、再開するという足場だけを押さえて先へ進めます。

### Router の状態と 'static

Axum 0.8 の `Router<S>` の構築・ルーティングで使う `impl` は `S: Clone + Send + Sync + 'static` を要求します。また `Handler` trait の関連型は `type Future: Future<Output = Response> + Send + 'static` です。このアプリでは `with_state(state)` へ渡す `AppState` と、`State<AppState>` を受け取る handler がその条件を満たす必要があります。

`AppState` は `Arc<ReadingService<SqliteBookRepository>>` を所有し、repository は `SqlitePool` を所有します。`build_app` のスタック上のローカル変数への参照を Router に残していません。ここでの `T: 'static` は「値が永遠に実行される・破棄されない」ではなく、非 static な参照に依存しない型の制約です。所有する `String` のような値も満たせますし、不要になれば通常どおり破棄されます。`&'static T` という参照そのものの有効期間とは区別してください。

共有状態や handler 全体の Future が `'static` でも、その内側で一時的に借りるすべての参照や repository の Future が `'static` である必要はありません。handler が所有する状態ハンドルや、service の Future が保持するローカル値を、その呼び出しの間だけ借用できます。外側の Future が外部の短命な参照に依存しないことと、実行中に内部で借用を使うことは両立します。

### Arc は共有所有を担当する

`Arc<T>` は参照カウントを原子的に管理し、複数の所有者が同じ `T` を保持できるようにします。内部の任意の `T` を自動でスレッド安全にする仕組みではありません。通常、`Arc<T>` が `Send + Sync` になるにも `T: Send + Sync` が必要です。

このアプリの共有先は、共有利用を想定した `SqlitePool` を持つ service です。[フェイクの章](../04-tests/service-fake.md)では、共有するメモリの変更を `Mutex` で保護します。`Arc` は所有者の共有、`Mutex` は排他的な変更と、役割を分けて読みます。

## 確認

1. `title` と `author` を借用する repository の Future に、無条件の `'static` は必要ですか。
2. Future が `&self` を待機中に保持するとき、repository の `Sync` はどう関係しますか。
3. `.await` のたびに別スレッドへ移動しますか。
4. `AppState: 'static` は、service が永遠に生存するという意味ですか。`Arc` だけで内部の安全性を保証できますか。

## 解答

1. 必要ありません。service の Future の中で借用先を保持して完了を待ちます。trait の Future にもその制約は付いていません。
2. `&R` をスレッド間で渡せるには `R: Sync` が必要なので、借用を保持する Future の `Send` を満たす条件になります。ほかの保持値も検査対象です。
3. いいえ。`Send` は移動可能性であり、移動の頻度や実行順の保証ではありません。
4. 非 static な参照に依存しないという意味で、破棄はできます。`Arc` は共有所有を提供しますが、内部の型も共有・移動の条件を満たす必要があります。
