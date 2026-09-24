# Future が値を保持する範囲

## 問い

一冊の読了記録を表す Future は何を所有し、何を借用して `.await` をまたぐのでしょうか。`Send`、`Sync`、`'static` の要求元も分けます。

## 前提

第 3 部まで読み、`ReadingService<R>` が repository を所有し、`BookRepository` の操作が Future を返すことを確認しているものとします。

## 読む場所と順序

1. `src/handler.rs` の `record_reading_completions`。
2. `src/service.rs` の `reading_completions_concurrency` と一冊を処理するメソッド。
3. `src/repository.rs` の `BookRepository: Send + Sync` と返り値の `+ Send`。
4. `src/app.rs` の `AppState<R>`。
5. Axum 0.8.9 の `Handler` と `Router<S>`、Tokio 1.53.1 の `spawn` の公開 Interface。

```rust,ignore
{{#include ../../../src/service.rs:reading_completions_concurrency}}
```

## 解説

`async fn` の呼び出しは処理を完了させず、Future を返します。実行器が poll し、内側の Future が `Pending` なら、再開に必要な値を保持して制御を返します。`.await` は新しいスレッドを作る命令でも、必ず停止する命令でもありません。

一括読了記録の親 Future は、検証済みの入力と `FuturesUnordered` を所有します。`async move` で作る各子 Future は、入力位置、`book_id`、`NoteBody` を所有します。子 Future は `&self` から service と repository を共有借用し、一冊の保存中は自分が所有する `NoteBody` を repository Future へ貸します。

```text
handler Future
  owns AppState<R>
  awaits parent service Future
    owns validated inputs and FuturesUnordered
      child Future
        owns position, book_id, NoteBody
        borrows ReadingService<R> and repository
        awaits repository Future borrowing NoteBody
```

repository の返り値は `impl Future<Output = Result<...>> + Send` で、無条件の `+ 'static` はありません。参照先を所有する子 Future の内側で完了まで待つため、repository Future はその参照の有効期間に結び付けられます。

`BookRepository: Sync` は、複数の子 Future が `&R` を共有し、その借用を待機中に保持できる条件に関係します。`BookRepository: Send` は repository を所有する service を移動可能にします。返る Future の `Send` は、handler まで入れ子になった処理の途中状態を移動可能にします。`Send` は実際に別スレッドで実行される保証ではありません。

`AppState<R>` は `Arc<ReadingService<R>>` を所有します。`Arc` は共有所有を提供しますが、内側の型を自動でスレッド安全にはしません。Router の状態には `Clone + Send + Sync + 'static` が要求され、repository の共有借用と Future には別途 `Sync` と `Send` が必要です。

## 別案との比較

### `tokio::spawn` で子 task を切り離す

`spawn` は Future に `Send + 'static` を要求します。今回は親 Future が全結果を待ち、終了時に未完了の子も終了する契約なので、task の所有者や永続的な処理状態を増やしません。

### `Arc<NoteBody>` を各子へ渡す

各本文は一つの子 Futureだけが使います。共有所有者を増やす必要がなく、`NoteBody` を値で移す方が小さく追えます。

### 借用した HTTP DTO を子 Future に残す

親が DTO を保持し続ければ成立する形もありますが、検証済みの値と未検証の入力の境界が曖昧になります。検証後の所有値へ切り替えます。

## 確認

1. 子 Future が所有する値と借用する値は何ですか。
2. repository Future に無条件の `'static` が不要なのはなぜですか。
3. `R: Sync` と返る Future の `Send` は、同じ条件ですか。
4. `Arc` だけで任意の repository を安全に共有できますか。
5. `tokio::spawn` を使うと追加で何を設計する必要がありますか。

## 解答

1. 入力位置、`book_id`、`NoteBody` を所有し、親が保持する service と repository を共有借用します。
2. 参照先を所有する子 Future の内側で repository Future を完了まで待ち、外へ切り離さないためです。
3. 違います。`Sync` は `&R` の共有に、Future の `Send` は待機中の状態全体の移動可能性に関係します。
4. できません。`Arc` は共有所有だけを担い、内側の `Send` と `Sync` の条件は残ります。
5. task の所有者、結果の再取得、shutdown、再試行、親の終了後と HTTP 応答の関係が必要です。

次は[独立した読了記録を並行に進める](concurrent-completions.md)で、これらの子 Future を同時に進行可能にする仕組みを読みます。
